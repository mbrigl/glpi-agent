// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-agent` — command-line entry point for the GLPI Agent Rust workspace
//! (v2.0.0).
//!
//! The first wired-up subcommand is `netdiscovery`, which scans IPv4 ranges and
//! prints the discovered devices as JSON (the other subcommands — inventory,
//! netinventory, esx, remoteinventory, inject, wakeup, daemon — land in later
//! phases). Logging honours `RUST_LOG` and is written to stderr so stdout stays
//! clean for the JSON result.

use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use glpi_core::types::snmp::SnmpCredentials;
use glpi_discovery::{Ipv4Range, NetDiscoveryTask, NetInventoryTask};
use glpi_http::{HttpServer, TrustList, DEFAULT_HTTP_PORT};
use glpi_scheduler::{jitter, RunSchedule};
use glpi_transport::{GlpiClient, Injector};
use tracing_subscriber::EnvFilter;

/// The GLPI Agent command-line interface.
#[derive(Parser)]
#[command(name = "glpi-agent", version, about = "GLPI Agent (Rust rewrite)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inventory the local machine and print it as JSON.
    Inventory,
    /// Scan IPv4 ranges for live and SNMP-capable devices (NetDiscovery).
    Netdiscovery(NetDiscoveryArgs),
    /// Inventory a single device over SNMP (NetInventory).
    Netinventory(NetInventoryArgs),
    /// Send existing inventory files (JSON/XML) to a GLPI server.
    Inject(InjectArgs),
    /// Run continuously: periodic NetDiscovery plus an HTTP control server.
    Daemon(DaemonArgs),
}

#[derive(Args)]
struct NetDiscoveryArgs {
    /// IPv4 targets: single (10.0.0.1), CIDR (10.0.0.0/24) or range (10.0.0.1-10).
    #[arg(required = true, value_name = "RANGE")]
    ranges: Vec<String>,

    /// SNMP v2c community to try (repeatable).
    #[arg(short = 'c', long = "community", value_name = "COMMUNITY")]
    communities: Vec<String>,

    /// Per-probe timeout in milliseconds.
    #[arg(long, default_value_t = 1000)]
    timeout_ms: u64,

    /// Maximum number of addresses probed concurrently.
    #[arg(long, default_value_t = 64)]
    concurrency: usize,

    /// Also resolve MAC addresses from the local ARP cache.
    #[arg(long)]
    arp: bool,
}

#[derive(Args)]
struct NetInventoryArgs {
    /// IP address of the device to inventory.
    #[arg(value_name = "TARGET")]
    target: IpAddr,

    /// SNMP v2c community to try (repeatable).
    #[arg(short = 'c', long = "community", value_name = "COMMUNITY")]
    communities: Vec<String>,

    /// Per-request timeout in milliseconds.
    #[arg(long, default_value_t = 2000)]
    timeout_ms: u64,

    /// Number of SNMP retries per request.
    #[arg(long, default_value_t = 0)]
    snmp_retries: u32,
}

#[derive(Args)]
struct InjectArgs {
    /// Inventory files to send (JSON or XML; format inferred from extension).
    #[arg(required = true, value_name = "FILE")]
    files: Vec<PathBuf>,

    /// GLPI server URL.
    #[arg(short = 's', long, value_name = "URL")]
    server: String,

    /// HTTP Basic auth username.
    #[arg(short = 'u', long)]
    user: Option<String>,

    /// HTTP Basic auth password.
    #[arg(short = 'p', long)]
    password: Option<String>,

    /// OAuth2 bearer token.
    #[arg(long, value_name = "TOKEN")]
    oauth_token: Option<String>,

    /// CA certificate file (PEM) for server verification.
    #[arg(long, value_name = "PATH")]
    ca_cert_file: Option<PathBuf>,

    /// Disable TLS certificate verification (insecure).
    #[arg(long)]
    no_ssl_check: bool,
}

#[derive(Args)]
struct DaemonArgs {
    /// IPv4 targets to re-scan each cycle (single / CIDR / range).
    #[arg(required = true, value_name = "RANGE")]
    ranges: Vec<String>,

    /// SNMP v2c community to try (repeatable).
    #[arg(short = 'c', long = "community", value_name = "COMMUNITY")]
    communities: Vec<String>,

    /// Seconds between scans.
    #[arg(long, default_value_t = 3600)]
    interval: u64,

    /// Maximum random delay (seconds) before the first scan.
    #[arg(long, default_value_t = 0)]
    delaytime: u64,

    /// Per-probe timeout in milliseconds.
    #[arg(long, default_value_t = 1000)]
    timeout_ms: u64,

    /// Disable the embedded HTTP control server.
    #[arg(long)]
    no_httpd: bool,

    /// Address the HTTP control server listens on.
    #[arg(long, default_value = "0.0.0.0")]
    httpd_ip: IpAddr,

    /// Port for the HTTP control server.
    #[arg(long, default_value_t = DEFAULT_HTTP_PORT)]
    httpd_port: u16,

    /// Comma-separated trusted clients (IPs / CIDRs) for the HTTP server.
    #[arg(long, default_value = "")]
    httpd_trust: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::Inventory => run_inventory().await,
        Command::Netdiscovery(args) => run_netdiscovery(args).await,
        Command::Netinventory(args) => run_netinventory(args).await,
        Command::Inject(args) => run_inject(args).await,
        Command::Daemon(args) => run_daemon(args).await,
    }
}

/// Inventories the local machine and prints the content as JSON.
async fn run_inventory() -> Result<()> {
    let content = glpi_inventory_local::LocalInventory::new().collect();
    println!("{}", serde_json::to_string_pretty(&content)?);
    Ok(())
}

/// Parses IPv4 range specs into [`Ipv4Range`]s.
fn parse_ranges(specs: &[String]) -> Result<Vec<Ipv4Range>> {
    specs
        .iter()
        .map(|spec| Ipv4Range::parse(spec).with_context(|| format!("invalid range {spec:?}")))
        .collect()
}

/// Builds v2c credentials from the given community strings.
fn v2c_credentials(communities: &[String]) -> Vec<SnmpCredentials> {
    communities
        .iter()
        .map(|community| SnmpCredentials::v2c(community.clone()))
        .collect()
}

/// Runs continuously: a periodic NetDiscovery scan plus an HTTP control server
/// that can trigger an immediate scan via `/now`. Stops on Ctrl-C.
async fn run_daemon(args: DaemonArgs) -> Result<()> {
    let task = NetDiscoveryTask::new(parse_ranges(&args.ranges)?)
        .with_credentials(v2c_credentials(&args.communities))
        .with_timeout(Duration::from_millis(args.timeout_ms));

    // HTTP control server (optional), yielding /now trigger events.
    let mut triggers = if args.no_httpd {
        None
    } else {
        let trust = TrustList::parse(args.httpd_trust.split(','))?;
        let status = format!("glpi-agent {}", env!("CARGO_PKG_VERSION"));
        let (server, rx) = HttpServer::new(args.httpd_ip, args.httpd_port, trust, status);
        tokio::spawn(async move {
            if let Err(err) = server.serve().await {
                tracing::error!(error = %err, "HTTP control server stopped");
            }
        });
        Some(rx)
    };

    let period = Duration::from_secs(args.interval);
    let initial_delay = jitter(
        Duration::from_secs(args.delaytime),
        pseudo_random_fraction(),
    );
    let mut schedule = RunSchedule::new(Utc::now(), period, initial_delay);
    tracing::info!(
        interval = args.interval,
        first_run_in = (schedule.next_run() - Utc::now()).num_seconds().max(0),
        "daemon started"
    );

    loop {
        let wait = (schedule.next_run() - Utc::now())
            .to_std()
            .unwrap_or(Duration::ZERO);

        tokio::select! {
            () = tokio::time::sleep(wait) => {
                scan_once(&task).await;
                schedule.schedule_next(Utc::now());
            }
            event = recv_trigger(&mut triggers) => {
                tracing::info!(?event, "received /now trigger");
                scan_once(&task).await;
                schedule.schedule_next(Utc::now());
            }
            result = tokio::signal::ctrl_c() => {
                result.context("waiting for Ctrl-C")?;
                tracing::info!("shutting down");
                return Ok(());
            }
        }
    }
}

/// Awaits the next `/now` trigger, or never resolves when the HTTP server is
/// disabled (so the `select!` arm stays dormant).
async fn recv_trigger(
    triggers: &mut Option<tokio::sync::mpsc::Receiver<glpi_http::NowRequest>>,
) -> glpi_http::NowRequest {
    match triggers {
        Some(rx) => match rx.recv().await {
            Some(event) => event,
            None => std::future::pending().await,
        },
        None => std::future::pending().await,
    }
}

/// Runs one NetDiscovery scan and logs the result count.
async fn scan_once(task: &NetDiscoveryTask) {
    tracing::info!(targets = task.target_count(), "scanning");
    let devices = task.run().await;
    match serde_json::to_string(&devices) {
        Ok(json) => println!("{json}"),
        Err(err) => tracing::error!(error = %err, "failed to serialize scan result"),
    }
    tracing::info!(count = devices.len(), "scan complete");
}

/// A cheap, dependency-free pseudo-random fraction in `[0, 1)` for jitter,
/// seeded from the current time's sub-second part.
fn pseudo_random_fraction() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos) / f64::from(u32::MAX)
}

/// Sends the given inventory files to a GLPI server.
async fn run_inject(args: InjectArgs) -> Result<()> {
    let mut builder = GlpiClient::builder(&args.server)
        .with_context(|| format!("invalid server URL {:?}", args.server))?;
    if let (Some(user), Some(password)) = (&args.user, &args.password) {
        builder = builder.basic_auth(user.clone(), password.clone());
    }
    if let Some(token) = &args.oauth_token {
        builder = builder.oauth_token(token.clone());
    }
    if let Some(ca) = &args.ca_cert_file {
        builder = builder.ca_cert_file(ca.clone());
    }
    let client = builder.no_ssl_check(args.no_ssl_check).build()?;

    let injector = Injector::new(client);
    for file in &args.files {
        injector
            .inject_file(file)
            .await
            .with_context(|| format!("injecting {}", file.display()))?;
        tracing::info!(file = %file.display(), "injected");
    }
    Ok(())
}

/// Inventories a single device and prints the result as JSON to stdout.
async fn run_netinventory(args: NetInventoryArgs) -> Result<()> {
    let task = NetInventoryTask::new(v2c_credentials(&args.communities))
        .with_timeout(Duration::from_millis(args.timeout_ms))
        .with_snmp_retries(args.snmp_retries);

    tracing::info!(target = %args.target, "starting NetInventory");
    match task.inventory(args.target).await? {
        Some(device) => {
            println!("{}", serde_json::to_string_pretty(&device)?);
            tracing::info!("device inventoried");
        }
        None => {
            tracing::warn!(target = %args.target, "no SNMP response from device");
            anyhow::bail!(
                "no SNMP response from {} with the given credentials",
                args.target
            );
        }
    }
    Ok(())
}

/// Runs the NetDiscovery scan and prints the result as JSON to stdout.
async fn run_netdiscovery(args: NetDiscoveryArgs) -> Result<()> {
    let task = NetDiscoveryTask::new(parse_ranges(&args.ranges)?)
        .with_credentials(v2c_credentials(&args.communities))
        .with_timeout(Duration::from_millis(args.timeout_ms))
        .with_concurrency(args.concurrency)
        .with_arp(args.arp);

    tracing::info!(targets = task.target_count(), "starting NetDiscovery scan");
    let devices = task.run().await;

    println!("{}", serde_json::to_string_pretty(&devices)?);
    tracing::info!(count = devices.len(), "scan complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_inventory_subcommand() {
        let cli = Cli::try_parse_from(["glpi-agent", "inventory"]).unwrap();
        assert!(matches!(cli.command, Command::Inventory));
    }

    #[test]
    fn parses_netdiscovery_with_options() {
        let cli = Cli::try_parse_from([
            "glpi-agent",
            "netdiscovery",
            "10.0.0.0/24",
            "192.168.1.1",
            "-c",
            "public",
            "--community",
            "private",
            "--timeout-ms",
            "500",
            "--concurrency",
            "32",
            "--arp",
        ])
        .unwrap();

        let Command::Netdiscovery(args) = cli.command else {
            panic!("expected netdiscovery");
        };
        assert_eq!(args.ranges, vec!["10.0.0.0/24", "192.168.1.1"]);
        assert_eq!(args.communities, vec!["public", "private"]);
        assert_eq!(args.timeout_ms, 500);
        assert_eq!(args.concurrency, 32);
        assert!(args.arp);
    }

    #[test]
    fn netdiscovery_requires_at_least_one_range() {
        assert!(Cli::try_parse_from(["glpi-agent", "netdiscovery"]).is_err());
    }

    #[test]
    fn netdiscovery_defaults_are_applied() {
        let cli = Cli::try_parse_from(["glpi-agent", "netdiscovery", "10.0.0.1"]).unwrap();
        let Command::Netdiscovery(args) = cli.command else {
            panic!("expected netdiscovery");
        };
        assert_eq!(args.timeout_ms, 1000);
        assert_eq!(args.concurrency, 64);
        assert!(!args.arp);
        assert!(args.communities.is_empty());
    }

    #[test]
    fn parses_netinventory_with_target_and_options() {
        let cli = Cli::try_parse_from([
            "glpi-agent",
            "netinventory",
            "10.0.0.5",
            "-c",
            "public",
            "--snmp-retries",
            "2",
        ])
        .unwrap();
        let Command::Netinventory(args) = cli.command else {
            panic!("expected netinventory");
        };
        assert_eq!(args.target, "10.0.0.5".parse::<std::net::IpAddr>().unwrap());
        assert_eq!(args.communities, vec!["public"]);
        assert_eq!(args.snmp_retries, 2);
        assert_eq!(args.timeout_ms, 2000);
    }

    #[test]
    fn netinventory_rejects_an_invalid_target() {
        assert!(Cli::try_parse_from(["glpi-agent", "netinventory", "not-an-ip"]).is_err());
    }

    #[test]
    fn parses_inject_with_server_and_auth() {
        let cli = Cli::try_parse_from([
            "glpi-agent",
            "inject",
            "a.json",
            "b.xml",
            "--server",
            "https://glpi.example/front/inventory.php",
            "-u",
            "agent",
            "-p",
            "secret",
            "--no-ssl-check",
        ])
        .unwrap();
        let Command::Inject(args) = cli.command else {
            panic!("expected inject");
        };
        assert_eq!(args.files.len(), 2);
        assert_eq!(args.server, "https://glpi.example/front/inventory.php");
        assert_eq!(args.user.as_deref(), Some("agent"));
        assert!(args.no_ssl_check);
    }

    #[test]
    fn inject_requires_server_and_files() {
        assert!(Cli::try_parse_from(["glpi-agent", "inject", "a.json"]).is_err());
        assert!(Cli::try_parse_from(["glpi-agent", "inject", "--server", "http://x"]).is_err());
    }

    #[test]
    fn parses_daemon_with_httpd_options() {
        let cli = Cli::try_parse_from([
            "glpi-agent",
            "daemon",
            "10.0.0.0/24",
            "-c",
            "public",
            "--interval",
            "900",
            "--httpd-port",
            "8080",
            "--httpd-trust",
            "192.168.0.0/16",
            "--no-httpd",
        ])
        .unwrap();
        let Command::Daemon(args) = cli.command else {
            panic!("expected daemon");
        };
        assert_eq!(args.ranges, vec!["10.0.0.0/24"]);
        assert_eq!(args.interval, 900);
        assert_eq!(args.httpd_port, 8080);
        assert_eq!(args.httpd_trust, "192.168.0.0/16");
        assert!(args.no_httpd);
    }

    #[test]
    fn daemon_defaults() {
        let cli = Cli::try_parse_from(["glpi-agent", "daemon", "10.0.0.1"]).unwrap();
        let Command::Daemon(args) = cli.command else {
            panic!("expected daemon");
        };
        assert_eq!(args.interval, 3600);
        assert_eq!(args.httpd_port, glpi_http::DEFAULT_HTTP_PORT);
        assert!(!args.no_httpd);
    }
}
