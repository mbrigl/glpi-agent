// SPDX-License-Identifier: GPL-2.0-only

//! `httpd-trust` access control for the embedded HTTP server.
//!
//! Only trusted clients may reach the control endpoints. A [`TrustList`] holds
//! the configured single addresses and IPv4 CIDR blocks; loopback is always
//! allowed (matching the upstream agent), so an empty list still permits local
//! control while denying everyone else.

use std::net::{IpAddr, Ipv4Addr};

use glpi_core::error::{AgentError, Result};

/// One entry of the trust list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TrustEntry {
    /// A single address (v4 or v6).
    Single(IpAddr),
    /// An IPv4 CIDR block: `(network, mask)`.
    V4Cidr(u32, u32),
}

/// The set of clients allowed to reach the HTTP control endpoints.
#[derive(Debug, Clone, Default)]
pub struct TrustList {
    entries: Vec<TrustEntry>,
}

impl TrustList {
    /// Parses trust entries (each a single IP or an IPv4 CIDR like
    /// `192.168.0.0/24`).
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if an entry is neither a valid IP address
    /// nor a valid IPv4 CIDR block.
    pub fn parse<'a>(entries: impl IntoIterator<Item = &'a str>) -> Result<Self> {
        let mut parsed = Vec::new();
        for raw in entries {
            let entry = raw.trim();
            if entry.is_empty() {
                continue;
            }
            parsed.push(parse_entry(entry)?);
        }
        Ok(Self { entries: parsed })
    }

    /// Returns `true` if `client` is allowed (loopback is always allowed).
    #[must_use]
    pub fn allows(&self, client: IpAddr) -> bool {
        if client.is_loopback() {
            return true;
        }
        self.entries.iter().any(|entry| match *entry {
            TrustEntry::Single(addr) => addr == client,
            TrustEntry::V4Cidr(network, mask) => match client {
                IpAddr::V4(v4) => (u32::from(v4) & mask) == network,
                IpAddr::V6(_) => false,
            },
        })
    }
}

/// Parses one trust entry: an IPv4 CIDR if it contains `/`, otherwise a single
/// IP address.
fn parse_entry(entry: &str) -> Result<TrustEntry> {
    if let Some((addr, prefix)) = entry.split_once('/') {
        let addr: Ipv4Addr = addr
            .trim()
            .parse()
            .map_err(|_| AgentError::Config(format!("invalid trust CIDR: {entry}")))?;
        let prefix: u32 = prefix
            .trim()
            .parse()
            .ok()
            .filter(|p| *p <= 32)
            .ok_or_else(|| AgentError::Config(format!("invalid CIDR prefix: {entry}")))?;
        let mask = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix)
        };
        Ok(TrustEntry::V4Cidr(u32::from(addr) & mask, mask))
    } else {
        entry
            .parse::<IpAddr>()
            .map(TrustEntry::Single)
            .map_err(|_| AgentError::Config(format!("invalid trust address: {entry}")))
    }
}

#[cfg(test)]
mod tests {
    use super::TrustList;
    use std::net::IpAddr;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn loopback_is_always_allowed() {
        let empty = TrustList::default();
        assert!(empty.allows(ip("127.0.0.1")));
        assert!(empty.allows(ip("::1")));
        assert!(!empty.allows(ip("10.0.0.1")));
    }

    #[test]
    fn single_addresses_match_exactly() {
        let trust = TrustList::parse(["10.0.0.5", "2001:db8::1"]).unwrap();
        assert!(trust.allows(ip("10.0.0.5")));
        assert!(trust.allows(ip("2001:db8::1")));
        assert!(!trust.allows(ip("10.0.0.6")));
    }

    #[test]
    fn cidr_blocks_match_the_subnet() {
        let trust = TrustList::parse(["192.168.1.0/24"]).unwrap();
        assert!(trust.allows(ip("192.168.1.1")));
        assert!(trust.allows(ip("192.168.1.254")));
        assert!(!trust.allows(ip("192.168.2.1")));
    }

    #[test]
    fn whitespace_and_blank_entries_are_tolerated() {
        let trust = TrustList::parse([" 10.0.0.0/8 ", "", "  "]).unwrap();
        assert!(trust.allows(ip("10.1.2.3")));
        assert!(!trust.allows(ip("11.0.0.1")));
    }

    #[test]
    fn malformed_entries_error() {
        assert!(TrustList::parse(["not-an-ip"]).is_err());
        assert!(TrustList::parse(["192.168.0.0/33"]).is_err());
        assert!(TrustList::parse(["10.0.0.0/x"]).is_err());
    }
}
