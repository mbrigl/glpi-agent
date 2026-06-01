// SPDX-License-Identifier: GPL-2.0-only

//! Core abstractions shared by the discovery methods and the scanner.
//!
//! A [`DiscoveryMethod`] probes a single address and reports whether the host
//! responded, along with any attributes it learned. The [`scanner`] runs many
//! methods across many addresses and merges their results into one
//! [`DiscoveredHost`] per responding address.
//!
//! [`scanner`]: crate::scanner

use std::net::IpAddr;

use async_trait::async_trait;
use glpi_core::error::Result;
use glpi_core::types::network::MacAddress;

/// Attributes a discovery method learned about a host that responded.
///
/// A method fills in only the fields it can determine — ping learns nothing
/// beyond reachability (an empty `Probe`), ARP supplies a [`mac`](Self::mac),
/// NetBIOS supplies a [`hostname`](Self::hostname). The scanner merges the
/// probes from every method into a single [`DiscoveredHost`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    /// The host's MAC address, if the method resolved one.
    pub mac: Option<MacAddress>,
    /// The host's name, if the method resolved one.
    pub hostname: Option<String>,
}

impl Probe {
    /// A probe that confirms reachability without any extra attributes.
    #[must_use]
    pub fn alive() -> Self {
        Self::default()
    }
}

/// A single technique for detecting and identifying a host at an address.
///
/// Implementations are the building blocks of a scan: ICMP ping, ARP-table
/// lookup, NetBIOS name query, SNMP. They must be cheap to share across tasks
/// (`Send + Sync`); the scanner clones an [`Arc`](std::sync::Arc) of each
/// method per probed address.
#[async_trait]
pub trait DiscoveryMethod: Send + Sync {
    /// A short, stable identifier recorded in [`DiscoveredHost::found_by`].
    fn name(&self) -> &'static str;

    /// Probes `target`.
    ///
    /// Returns `Ok(Some(probe))` if the host responded, `Ok(None)` if it did
    /// not (down, or the method does not apply), and `Err` only on a genuine
    /// method failure — the scanner logs errors and treats them as "no
    /// response" rather than aborting the scan.
    ///
    /// # Errors
    ///
    /// Returns an error if the probe could not be carried out (for example a
    /// socket could not be opened).
    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>>;
}

/// A host that responded to at least one discovery method during a scan.
///
/// Produced by [`Scanner::scan`](crate::scanner::Scanner::scan), with the
/// attributes from every method that found it merged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredHost {
    /// The address that responded.
    pub address: IpAddr,
    /// The merged MAC address, if any method resolved one.
    pub mac: Option<MacAddress>,
    /// The merged hostname, if any method resolved one.
    pub hostname: Option<String>,
    /// Names of the methods that detected this host, in probe order.
    pub found_by: Vec<&'static str>,
}

impl DiscoveredHost {
    /// Creates an as-yet-undetected host record for `address`.
    fn new(address: IpAddr) -> Self {
        Self {
            address,
            mac: None,
            hostname: None,
            found_by: Vec::new(),
        }
    }

    /// Folds a successful `probe` from method `name` into this record.
    ///
    /// The first non-empty value wins for each attribute, so methods probed
    /// earlier take precedence over later ones.
    fn merge(&mut self, name: &'static str, probe: Probe) {
        self.found_by.push(name);
        if self.mac.is_none() {
            self.mac = probe.mac;
        }
        if self.hostname.is_none() {
            self.hostname = probe.hostname;
        }
    }
}

/// Runs every `method` against `target` in order and merges the responses.
///
/// Returns `None` if no method found the host. Each probe is bounded by
/// `timeout`; a method that times out or errors is logged and treated as no
/// response. This is the per-address work the scanner schedules in parallel.
pub(crate) async fn probe_address(
    target: IpAddr,
    methods: &[std::sync::Arc<dyn DiscoveryMethod>],
    timeout: std::time::Duration,
) -> Option<DiscoveredHost> {
    let mut host = DiscoveredHost::new(target);
    for method in methods {
        let name = method.name();
        match tokio::time::timeout(timeout, method.probe(target)).await {
            Ok(Ok(Some(probe))) => host.merge(name, probe),
            Ok(Ok(None)) => {}
            Ok(Err(err)) => {
                tracing::debug!(%target, method = name, error = %err, "discovery method failed");
            }
            Err(_) => {
                tracing::debug!(%target, method = name, "discovery method timed out");
            }
        }
    }
    (!host.found_by.is_empty()).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::{DiscoveredHost, Probe};
    use glpi_core::types::network::MacAddress;
    use std::net::{IpAddr, Ipv4Addr};

    fn ip(d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 0, d))
    }

    #[test]
    fn merge_records_method_order_and_first_value_wins() {
        let mut host = DiscoveredHost::new(ip(1));
        host.merge(
            "arp",
            Probe {
                mac: Some(MacAddress::new([0, 1, 2, 3, 4, 5])),
                hostname: None,
            },
        );
        host.merge(
            "netbios",
            Probe {
                mac: Some(MacAddress::new([9, 9, 9, 9, 9, 9])),
                hostname: Some("host-a".to_owned()),
            },
        );
        assert_eq!(host.found_by, vec!["arp", "netbios"]);
        // First method's MAC is kept; NetBIOS only fills the still-empty hostname.
        assert_eq!(host.mac, Some(MacAddress::new([0, 1, 2, 3, 4, 5])));
        assert_eq!(host.hostname.as_deref(), Some("host-a"));
    }

    #[test]
    fn alive_probe_carries_no_attributes() {
        assert_eq!(
            Probe::alive(),
            Probe {
                mac: None,
                hostname: None
            }
        );
    }
}
