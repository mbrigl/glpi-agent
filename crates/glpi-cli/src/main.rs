// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-agent` — command-line entry point for the GLPI Agent Rust workspace
//! (v2.0.0).
//!
//! The first wired-up subcommand is `netdiscovery`, which scans IPv4 ranges and
//! prints the discovered devices as JSON (the other subcommands — inventory,
//! netinventory, esx, remoteinventory, inject, wakeup, daemon — land in later
//! phases). Logging honours `RUST_LOG` and is written to stderr so stdout stays
//! clean for the JSON result.

use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use glpi_core::types::snmp::SnmpCredentials;
use glpi_discovery::{Ipv4Range, NetDiscoveryTask};
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
    /// Scan IPv4 ranges for live and SNMP-capable devices (NetDiscovery).
    Netdiscovery(NetDiscoveryArgs),
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::Netdiscovery(args) => run_netdiscovery(args).await,
    }
}

/// Runs the NetDiscovery scan and prints the result as JSON to stdout.
async fn run_netdiscovery(args: NetDiscoveryArgs) -> Result<()> {
    let ranges = args
        .ranges
        .iter()
        .map(|spec| Ipv4Range::parse(spec).with_context(|| format!("invalid range {spec:?}")))
        .collect::<Result<Vec<_>>>()?;

    let credentials = args
        .communities
        .iter()
        .map(|community| SnmpCredentials::v2c(community.clone()))
        .collect();

    let task = NetDiscoveryTask::new(ranges)
        .with_credentials(credentials)
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

        let Command::Netdiscovery(args) = cli.command;
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
        let Command::Netdiscovery(args) = cli.command;
        assert_eq!(args.timeout_ms, 1000);
        assert_eq!(args.concurrency, 64);
        assert!(!args.arp);
        assert!(args.communities.is_empty());
    }
}
