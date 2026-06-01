// SPDX-License-Identifier: GPL-2.0-only

//! Integration tests for the parallel [`Scanner`] using mock discovery methods.
//!
//! These run entirely offline: the mock methods stand in for ping / ARP /
//! NetBIOS / SNMP so the scanner's concurrency, timeout, error handling and
//! result merging can be exercised without any network access.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};
use glpi_core::types::network::MacAddress;
use glpi_discovery::ip_range::Ipv4Range;
use glpi_discovery::scanner::{ScanProgress, Scanner};
use glpi_discovery::traits::{DiscoveryMethod, Probe};

/// Responds for the given last-octet set, optionally supplying a MAC address.
struct AliveOn {
    name: &'static str,
    alive: Vec<u8>,
    mac: Option<MacAddress>,
}

#[async_trait]
impl DiscoveryMethod for AliveOn {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>> {
        let IpAddr::V4(v4) = target else {
            return Ok(None);
        };
        if self.alive.contains(&v4.octets()[3]) {
            Ok(Some(Probe {
                mac: self.mac,
                hostname: None,
            }))
        } else {
            Ok(None)
        }
    }
}

/// Sleeps past any reasonable per-probe timeout before it would respond.
struct TooSlow;

#[async_trait]
impl DiscoveryMethod for TooSlow {
    fn name(&self) -> &'static str {
        "slow"
    }

    async fn probe(&self, _target: IpAddr) -> Result<Option<Probe>> {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok(Some(Probe::alive()))
    }
}

/// Always fails; the scanner must treat this as "no response".
struct AlwaysErrors;

#[async_trait]
impl DiscoveryMethod for AlwaysErrors {
    fn name(&self) -> &'static str {
        "boom"
    }

    async fn probe(&self, _target: IpAddr) -> Result<Option<Probe>> {
        Err(AgentError::Task("probe exploded".to_owned()))
    }
}

fn v4(d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, d))
}

fn targets() -> impl Iterator<Item = IpAddr> {
    Ipv4Range::parse("10.0.0.0/29")
        .unwrap()
        .iter()
        .map(IpAddr::V4)
}

#[tokio::test]
async fn returns_only_responding_hosts_sorted() {
    let methods: Vec<Arc<dyn DiscoveryMethod>> = vec![Arc::new(AliveOn {
        name: "ping",
        alive: vec![5, 1, 3],
        mac: None,
    })];
    let scanner = Scanner::new(4, Duration::from_secs(1));
    let hosts = scanner.scan(targets(), &methods).await;

    let addrs: Vec<_> = hosts.iter().map(|h| h.address).collect();
    assert_eq!(addrs, vec![v4(1), v4(3), v4(5)]);
    assert!(hosts.iter().all(|h| h.found_by == vec!["ping"]));
}

#[tokio::test]
async fn merges_attributes_across_methods() {
    let mac = MacAddress::new([0xde, 0xad, 0xbe, 0xef, 0x00, 0x01]);
    let methods: Vec<Arc<dyn DiscoveryMethod>> = vec![
        Arc::new(AliveOn {
            name: "ping",
            alive: vec![2],
            mac: None,
        }),
        Arc::new(AliveOn {
            name: "arp",
            alive: vec![2],
            mac: Some(mac),
        }),
    ];
    let scanner = Scanner::new(8, Duration::from_secs(1));
    let hosts = scanner.scan(targets(), &methods).await;

    assert_eq!(hosts.len(), 1);
    let host = &hosts[0];
    assert_eq!(host.address, v4(2));
    assert_eq!(host.found_by, vec!["ping", "arp"]);
    assert_eq!(host.mac, Some(mac));
}

#[tokio::test]
async fn slow_method_times_out_without_dropping_a_fast_hit() {
    let methods: Vec<Arc<dyn DiscoveryMethod>> = vec![
        Arc::new(TooSlow),
        Arc::new(AliveOn {
            name: "ping",
            alive: vec![4],
            mac: None,
        }),
    ];
    // Short timeout: the slow method is abandoned, the fast one still hits.
    let scanner = Scanner::new(8, Duration::from_millis(50));
    let hosts = scanner.scan(targets(), &methods).await;

    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].address, v4(4));
    assert_eq!(hosts[0].found_by, vec!["ping"]);
}

#[tokio::test]
async fn erroring_method_is_treated_as_no_response() {
    let methods: Vec<Arc<dyn DiscoveryMethod>> = vec![Arc::new(AlwaysErrors)];
    let scanner = Scanner::new(4, Duration::from_secs(1));
    let hosts = scanner.scan(targets(), &methods).await;
    assert!(hosts.is_empty());
}

#[tokio::test]
async fn progress_callback_fires_once_per_address() {
    let seen = Arc::new(AtomicU64::new(0));
    let last_total = Arc::new(AtomicU64::new(0));
    let seen_cb = Arc::clone(&seen);
    let total_cb = Arc::clone(&last_total);

    let methods: Vec<Arc<dyn DiscoveryMethod>> = vec![Arc::new(AliveOn {
        name: "ping",
        alive: vec![],
        mac: None,
    })];
    let scanner =
        Scanner::new(2, Duration::from_secs(1)).with_progress(Arc::new(move |p: ScanProgress| {
            seen_cb.fetch_add(1, Ordering::SeqCst);
            total_cb.store(p.total, Ordering::SeqCst);
        }));

    scanner.scan(targets(), &methods).await;

    // /29 has 8 addresses: the callback fires exactly eight times, each
    // reporting a total of eight.
    assert_eq!(seen.load(Ordering::SeqCst), 8);
    assert_eq!(last_total.load(Ordering::SeqCst), 8);
}
