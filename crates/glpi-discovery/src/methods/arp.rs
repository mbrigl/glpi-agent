// SPDX-License-Identifier: GPL-2.0-only

//! ARP-cache discovery method.
//!
//! ARP does not actively probe: it consults the host's existing ARP cache and
//! reports the MAC address of any target already present there (typically a
//! host that responded to an earlier ping). It is a supplementary method that
//! enriches a [`DiscoveredHost`](crate::traits::DiscoveredHost) with a MAC
//! address rather than one that detects new hosts on its own.
//!
//! The cache is read once when the method is built — on Linux from
//! `/proc/net/arp`, elsewhere from the output of `arp -a` — and parsed by a
//! single format-tolerant [`ArpTable::parse`] that handles the Linux, BSD /
//! macOS and Windows layouts. The parser is the unit-tested core; the live
//! read is a thin platform wrapper around it.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use async_trait::async_trait;
use glpi_core::error::Result;
use glpi_core::types::network::MacAddress;

use crate::traits::{DiscoveryMethod, Probe};

/// A snapshot of the system ARP cache: IPv4 address → MAC address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArpTable {
    entries: HashMap<Ipv4Addr, MacAddress>,
}

impl ArpTable {
    /// Parses an ARP table from any of the common textual layouts.
    ///
    /// One entry is taken per line: the first token that parses as an IPv4
    /// address and the first that parses as a MAC address. Lines without both
    /// (headers, `incomplete` entries) are skipped, as are entries whose MAC is
    /// all-zero (an unresolved `/proc/net/arp` row). This single pass handles:
    ///
    /// * Linux `/proc/net/arp` (column layout),
    /// * Linux / BSD / macOS `arp -a` (`host (ip) at mac ...`),
    /// * Windows `arp -a` (`ip   physical-address   type`, hyphen-separated).
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut entries = HashMap::new();
        for line in text.lines() {
            if let (Some(ip), Some(mac)) = (find_ipv4(line), find_mac(line)) {
                if mac.octets() != [0; 6] {
                    entries.insert(ip, mac);
                }
            }
        }
        Self { entries }
    }

    /// Builds a table directly from address/MAC pairs (used in tests).
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (Ipv4Addr, MacAddress)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Reads and parses the live system ARP cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache source cannot be read (the `/proc/net/arp`
    /// file on Linux, or the `arp -a` command elsewhere).
    pub fn load() -> Result<Self> {
        Ok(Self::parse(&read_system_arp()?))
    }

    /// Returns the cached MAC address for `ip`, if present.
    #[must_use]
    pub fn get(&self, ip: Ipv4Addr) -> Option<MacAddress> {
        self.entries.get(&ip).copied()
    }

    /// Returns the number of resolved entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the table has no resolved entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Discovery method that resolves MAC addresses from a captured [`ArpTable`].
///
/// Build it from the live cache with [`ArpMethod::from_system`], or from an
/// explicit table with [`ArpMethod::with_table`]. Probing is a pure lookup —
/// the cache is read once at construction, so no I/O happens per address.
pub struct ArpMethod {
    table: ArpTable,
}

impl ArpMethod {
    /// Builds the method from the live system ARP cache.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`ArpTable::load`].
    pub fn from_system() -> Result<Self> {
        Ok(Self {
            table: ArpTable::load()?,
        })
    }

    /// Builds the method from an already-loaded table.
    #[must_use]
    pub fn with_table(table: ArpTable) -> Self {
        Self { table }
    }
}

#[async_trait]
impl DiscoveryMethod for ArpMethod {
    fn name(&self) -> &'static str {
        "arp"
    }

    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>> {
        let IpAddr::V4(v4) = target else {
            return Ok(None);
        };
        Ok(self.table.get(v4).map(|mac| Probe {
            mac: Some(mac),
            hostname: None,
        }))
    }
}

/// Returns the first whitespace token of `line` that parses as an IPv4 address,
/// after stripping surrounding brackets and punctuation (`arp -a` wraps the
/// address in parentheses).
fn find_ipv4(line: &str) -> Option<Ipv4Addr> {
    line.split_whitespace()
        .map(|tok| tok.trim_matches(|c: char| "()[]{}<>,;".contains(c)))
        .find_map(|tok| Ipv4Addr::from_str(tok).ok())
}

/// Returns the first whitespace token of `line` that parses as a MAC address.
///
/// [`MacAddress::from_str`] requires exactly six `:`/`-`-separated hex groups,
/// so non-MAC tokens (`0x1`, `dynamic`, `[ether]`, an IPv4 literal) are
/// rejected without a separate guard.
fn find_mac(line: &str) -> Option<MacAddress> {
    line.split_whitespace()
        .find_map(|tok| MacAddress::from_str(tok).ok())
}

#[cfg(target_os = "linux")]
fn read_system_arp() -> Result<String> {
    Ok(std::fs::read_to_string("/proc/net/arp")?)
}

#[cfg(not(target_os = "linux"))]
fn read_system_arp() -> Result<String> {
    use glpi_core::error::AgentError;

    let output = std::process::Command::new("arp")
        .arg("-a")
        .output()
        .map_err(|e| AgentError::Task(format!("failed to run `arp -a`: {e}")))?;
    if !output.status.success() {
        return Err(AgentError::Task(format!(
            "`arp -a` exited with {}",
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{ArpMethod, ArpTable};
    use crate::traits::DiscoveryMethod;
    use glpi_core::types::network::MacAddress;
    use std::net::{IpAddr, Ipv4Addr};

    const PROC_NET_ARP: &str = "\
IP address       HW type     Flags       HW address            Mask     Device
192.168.1.1      0x1         0x2         00:1a:2b:3c:4d:5e     *        eth0
192.168.1.2      0x1         0x0         00:00:00:00:00:00     *        eth0
192.168.1.3      0x1         0x2         aa:bb:cc:dd:ee:ff     *        eth0
";

    const ARP_A_BSD: &str = "\
? (192.168.1.1) at 0:1a:2b:3c:4d:5e on en0 ifscope [ethernet]
host.example.com (192.168.1.4) at a:b:c:d:e:f on en0 ifscope [ethernet]
? (192.168.1.9) at (incomplete) on en0 ifscope [ethernet]
";

    const ARP_A_WINDOWS: &str = "\
Interface: 192.168.1.10 --- 0x2
  Internet Address      Physical Address      Type
  192.168.1.1           00-1a-2b-3c-4d-5e     dynamic
  192.168.1.255         ff-ff-ff-ff-ff-ff     static
";

    #[test]
    fn parses_proc_net_arp_skipping_header_and_incomplete() {
        let table = ArpTable::parse(PROC_NET_ARP);
        assert_eq!(table.len(), 2);
        assert_eq!(
            table.get(Ipv4Addr::new(192, 168, 1, 1)),
            Some(MacAddress::new([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]))
        );
        assert_eq!(
            table.get(Ipv4Addr::new(192, 168, 1, 3)),
            Some(MacAddress::new([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]))
        );
        // The 0x0-flag / all-zero entry is dropped.
        assert_eq!(table.get(Ipv4Addr::new(192, 168, 1, 2)), None);
    }

    #[test]
    fn parses_bsd_arp_a_with_short_hex_and_incomplete() {
        let table = ArpTable::parse(ARP_A_BSD);
        assert_eq!(table.len(), 2);
        assert_eq!(
            table.get(Ipv4Addr::new(192, 168, 1, 1)),
            Some(MacAddress::new([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]))
        );
        assert_eq!(
            table.get(Ipv4Addr::new(192, 168, 1, 4)),
            Some(MacAddress::new([0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f]))
        );
        // The "(incomplete)" entry has no MAC token.
        assert_eq!(table.get(Ipv4Addr::new(192, 168, 1, 9)), None);
    }

    #[test]
    fn parses_windows_arp_a_with_hyphen_separators() {
        let table = ArpTable::parse(ARP_A_WINDOWS);
        // The interface header line has an IP but no MAC, so it is not an entry.
        assert_eq!(
            table.get(Ipv4Addr::new(192, 168, 1, 1)),
            Some(MacAddress::new([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]))
        );
        assert_eq!(
            table.get(Ipv4Addr::new(192, 168, 1, 255)),
            Some(MacAddress::new([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]))
        );
        assert_eq!(table.get(Ipv4Addr::new(192, 168, 1, 10)), None);
    }

    #[tokio::test]
    async fn method_returns_mac_for_cached_host_and_none_otherwise() {
        let method = ArpMethod::with_table(ArpTable::parse(PROC_NET_ARP));
        let hit = method
            .probe(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))
            .await
            .unwrap();
        assert_eq!(
            hit.and_then(|p| p.mac),
            Some(MacAddress::new([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]))
        );

        let miss = method
            .probe(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 99)))
            .await
            .unwrap();
        assert!(miss.is_none());
    }

    #[tokio::test]
    async fn method_ignores_ipv6_targets() {
        let method = ArpMethod::with_table(ArpTable::default());
        let result = method.probe("::1".parse().unwrap()).await.unwrap();
        assert!(result.is_none());
    }
}
