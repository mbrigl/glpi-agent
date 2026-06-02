// SPDX-License-Identifier: GPL-2.0-only

//! P2P mirror peer enumeration.
//!
//! When peer-to-peer download is enabled, the agent looks for the file on other
//! agents in its local network before falling back to the server. This module
//! enumerates the candidate peer addresses for a network, deliberately
//! **excluding the network and broadcast addresses** (scanning them is both
//! useless and antisocial), and caps the range so a wide CIDR cannot fan out
//! into a scan of thousands of hosts.

use std::net::{Ipv4Addr, SocketAddr};

use glpi_core::error::{AgentError, Result};

/// The widest network the peer scan will enumerate (a `/20` ⇒ 4094 hosts).
const MIN_PREFIX_LEN: u8 = 20;

/// Returns the candidate peer socket addresses for `cidr` (e.g. `10.0.0.0/24`)
/// on `port`, excluding the network and broadcast addresses.
///
/// # Errors
///
/// Returns [`AgentError::Parse`] for a malformed CIDR, or [`AgentError::Task`]
/// if the network is wider than [`MIN_PREFIX_LEN`].
pub fn peer_candidates(cidr: &str, port: u16) -> Result<Vec<SocketAddr>> {
    let (addr, prefix) = parse_cidr(cidr)?;
    if prefix < MIN_PREFIX_LEN {
        return Err(AgentError::Task(format!(
            "network {cidr} is too wide for a peer scan (prefix < /{MIN_PREFIX_LEN})"
        )));
    }
    let base = u32::from(addr);
    let mask: u32 = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let network = base & mask;
    let broadcast = network | !mask;

    // A /31 or /32 has no usable host range between network and broadcast.
    if broadcast.saturating_sub(network) < 2 {
        return Ok(Vec::new());
    }

    Ok((network + 1..broadcast)
        .map(|host| SocketAddr::from((Ipv4Addr::from(host), port)))
        .collect())
}

/// Parses `a.b.c.d/len` into its address and prefix length.
fn parse_cidr(cidr: &str) -> Result<(Ipv4Addr, u8)> {
    let (addr, prefix) = cidr
        .split_once('/')
        .ok_or_else(|| AgentError::Parse(format!("not a CIDR: {cidr}")))?;
    let addr: Ipv4Addr = addr
        .parse()
        .map_err(|_| AgentError::Parse(format!("invalid IPv4 address in {cidr}")))?;
    let prefix: u8 = prefix
        .parse()
        .ok()
        .filter(|p| *p <= 32)
        .ok_or_else(|| AgentError::Parse(format!("invalid prefix in {cidr}")))?;
    Ok((addr, prefix))
}

#[cfg(test)]
mod tests {
    use super::peer_candidates;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn excludes_network_and_broadcast() {
        let peers = peer_candidates("10.0.0.0/24", 62354).unwrap();
        assert_eq!(peers.len(), 254);
        let ips: Vec<IpAddr> = peers.iter().map(|s| s.ip()).collect();
        assert!(
            !ips.contains(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0))),
            "network excluded"
        );
        assert!(
            !ips.contains(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 255))),
            "broadcast excluded"
        );
        assert_eq!(peers[0].ip(), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(peers[0].port(), 62354);
    }

    #[test]
    fn computes_network_from_any_host_address() {
        // A host address inside the network yields the same candidate set.
        let peers = peer_candidates("192.168.1.42/24", 9).unwrap();
        assert_eq!(peers.len(), 254);
        assert_eq!(peers[0].ip(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn rejects_too_wide_a_network() {
        assert!(peer_candidates("10.0.0.0/8", 9).is_err());
    }

    #[test]
    fn small_networks_have_no_usable_hosts() {
        assert!(peer_candidates("10.0.0.0/31", 9).unwrap().is_empty());
        assert!(peer_candidates("10.0.0.1/32", 9).unwrap().is_empty());
    }

    #[test]
    fn rejects_malformed_cidr() {
        assert!(peer_candidates("not-a-cidr", 9).is_err());
        assert!(peer_candidates("10.0.0.0/40", 9).is_err());
    }
}
