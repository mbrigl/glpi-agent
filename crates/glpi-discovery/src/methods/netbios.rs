// SPDX-License-Identifier: GPL-2.0-only

//! NetBIOS name-service discovery method (UDP port 137).
//!
//! Sends a NetBIOS *node status request* (RFC 1002 §4.2.17) to the target and
//! parses the *node status response* to recover the machine's registered
//! names. The first unique workstation name (suffix `0x00`, group bit clear)
//! is reported as the host's [`hostname`](crate::traits::Probe::hostname).
//!
//! The query carries the wildcard name `*`, encoded with the RFC 1001
//! "second-level" half-ASCII scheme (each octet split into two nibbles, each
//! mapped to `A..P`). Query construction and response parsing are pure and
//! unit-tested; only [`NetBiosMethod::probe`] touches the network.

use std::net::IpAddr;
use std::time::Duration;

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};
use tokio::net::UdpSocket;

use crate::traits::{DiscoveryMethod, Probe};

/// UDP port of the NetBIOS name service.
pub const NETBIOS_NS_PORT: u16 = 137;

/// QTYPE for a node status request (`NBSTAT`).
const QTYPE_NBSTAT: u16 = 0x0021;
/// QCLASS for the internet class (`IN`).
const QCLASS_IN: u16 = 0x0001;
/// Bytes per name entry in the response RDATA: 15-byte name + suffix + 2 flags.
const NAME_ENTRY_LEN: usize = 18;
/// GROUP bit in a name entry's flags field.
const GROUP_FLAG: u16 = 0x8000;

/// A NetBIOS name registered by a node, as returned in a node status response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetBiosName {
    /// The name with trailing padding removed.
    pub name: String,
    /// The one-byte suffix identifying the service (e.g. `0x00` = workstation).
    pub suffix: u8,
    /// `true` if this is a group (shared) name rather than a unique one.
    pub group: bool,
}

/// Builds a node status request for the wildcard name `*`.
///
/// `transaction_id` is echoed by the responder; the agent does not rely on it
/// for matching (one query per socket), but a distinct value aids packet
/// captures.
#[must_use]
pub fn build_node_status_query(transaction_id: u16) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(50);
    pkt.extend_from_slice(&transaction_id.to_be_bytes());
    pkt.extend_from_slice(&0x0000u16.to_be_bytes()); // flags: query, no recursion
    pkt.extend_from_slice(&0x0001u16.to_be_bytes()); // QDCOUNT
    pkt.extend_from_slice(&0x0000u16.to_be_bytes()); // ANCOUNT
    pkt.extend_from_slice(&0x0000u16.to_be_bytes()); // NSCOUNT
    pkt.extend_from_slice(&0x0000u16.to_be_bytes()); // ARCOUNT

    let encoded = encode_netbios_name(b'*');
    pkt.push(encoded.len() as u8); // label length (0x20)
    pkt.extend_from_slice(&encoded);
    pkt.push(0x00); // root label terminator
    pkt.extend_from_slice(&QTYPE_NBSTAT.to_be_bytes());
    pkt.extend_from_slice(&QCLASS_IN.to_be_bytes());
    pkt
}

/// Half-ASCII-encodes the wildcard name: `first` byte followed by 15 NULs, each
/// octet split into two nibbles mapped onto `A..P`. Always 32 bytes.
fn encode_netbios_name(first: u8) -> [u8; 32] {
    let mut raw = [0u8; 16];
    raw[0] = first;
    let mut out = [0u8; 32];
    for (i, byte) in raw.iter().enumerate() {
        out[i * 2] = b'A' + (byte >> 4);
        out[i * 2 + 1] = b'A' + (byte & 0x0f);
    }
    out
}

/// Parses a node status response into the list of registered names.
///
/// # Errors
///
/// Returns [`AgentError::Protocol`] if the message is truncated or otherwise
/// not a well-formed node status response. Parsing is bounds-checked
/// throughout, so malformed input never panics.
pub fn parse_node_status_response(data: &[u8]) -> Result<Vec<NetBiosName>> {
    let err = || AgentError::Protocol("malformed NetBIOS node status response".to_owned());

    if data.len() < 12 {
        return Err(err());
    }
    // Skip the 12-byte header, then the answer RR name, TYPE, CLASS, TTL and
    // RDLENGTH to reach the RDATA.
    let mut off = skip_name(data, 12).ok_or_else(err)?;
    off = off.checked_add(10).ok_or_else(err)?; // TYPE(2)+CLASS(2)+TTL(4)+RDLENGTH(2)
    let num_names = *data.get(off).ok_or_else(err)?;
    off += 1;

    let mut names = Vec::with_capacity(usize::from(num_names));
    for _ in 0..num_names {
        let entry = data.get(off..off + NAME_ENTRY_LEN).ok_or_else(err)?;
        let name = String::from_utf8_lossy(&entry[0..15])
            .trim_end_matches([' ', '\0'])
            .to_owned();
        let suffix = entry[15];
        let flags = u16::from_be_bytes([entry[16], entry[17]]);
        names.push(NetBiosName {
            name,
            suffix,
            group: flags & GROUP_FLAG != 0,
        });
        off += NAME_ENTRY_LEN;
    }
    Ok(names)
}

/// Returns the host's workstation name: the first unique name with suffix
/// `0x00` (and a non-empty label).
#[must_use]
pub fn workstation_name(names: &[NetBiosName]) -> Option<String> {
    names
        .iter()
        .find(|n| n.suffix == 0x00 && !n.group && !n.name.is_empty())
        .map(|n| n.name.clone())
}

/// Advances past a domain name starting at `offset`, returning the offset just
/// after it. Handles a sequence of length-prefixed labels, the root
/// terminator, and a compression pointer (top two bits set).
fn skip_name(data: &[u8], mut offset: usize) -> Option<usize> {
    loop {
        let len = *data.get(offset)?;
        if len == 0 {
            return Some(offset + 1);
        }
        if len & 0xc0 == 0xc0 {
            // A two-byte pointer ends the name.
            return offset.checked_add(2).filter(|&o| o <= data.len());
        }
        offset = offset.checked_add(1 + usize::from(len))?;
    }
}

/// Discovery method that queries the NetBIOS name service for a host's name.
#[derive(Debug, Clone)]
pub struct NetBiosMethod {
    timeout: Duration,
}

impl NetBiosMethod {
    /// Creates a method whose single request/response exchange is bounded by
    /// `timeout`.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl DiscoveryMethod for NetBiosMethod {
    fn name(&self) -> &'static str {
        "netbios"
    }

    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>> {
        // NetBIOS name service is IPv4-only.
        if !target.is_ipv4() {
            return Ok(None);
        }
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let query = build_node_status_query(0x4747);
        socket.send_to(&query, (target, NETBIOS_NS_PORT)).await?;

        let mut buf = [0u8; 1024];
        let received = match tokio::time::timeout(self.timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, _))) => n,
            // Timeout or socket error: treat as no response.
            _ => return Ok(None),
        };

        let names = parse_node_status_response(&buf[..received])?;
        Ok(workstation_name(&names).map(|hostname| Probe {
            mac: None,
            hostname: Some(hostname),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_node_status_query, encode_netbios_name, parse_node_status_response, workstation_name,
        QCLASS_IN, QTYPE_NBSTAT,
    };

    #[test]
    fn wildcard_name_encodes_to_ckaa_block() {
        let encoded = encode_netbios_name(b'*');
        // '*' = 0x2A -> nibbles 0x2,0xA -> 'C','K'; NUL -> 'A','A'.
        assert_eq!(&encoded[0..2], b"CK");
        assert!(encoded[2..].iter().all(|&b| b == b'A'));
        assert_eq!(encoded.len(), 32);
    }

    #[test]
    fn query_has_expected_header_and_question() {
        let pkt = build_node_status_query(0x1234);
        assert_eq!(pkt.len(), 50);
        assert_eq!(&pkt[0..2], &[0x12, 0x34]); // transaction id
        assert_eq!(&pkt[4..6], &[0x00, 0x01]); // QDCOUNT = 1
        assert_eq!(pkt[12], 0x20); // label length
        assert_eq!(&pkt[13..15], b"CK"); // start of encoded wildcard name
        assert_eq!(pkt[45], 0x00); // root terminator
        assert_eq!(&pkt[46..48], &QTYPE_NBSTAT.to_be_bytes());
        assert_eq!(&pkt[48..50], &QCLASS_IN.to_be_bytes());
    }

    /// Builds a node status response carrying the given `(name, suffix, group)`
    /// entries, reusing the 34-byte encoded wildcard name as the answer name.
    fn build_response(entries: &[(&str, u8, bool)]) -> Vec<u8> {
        let mut pkt = vec![0x12, 0x34]; // transaction id
        pkt.extend_from_slice(&[0x84, 0x00]); // flags: response, authoritative
        pkt.extend_from_slice(&[0x00, 0x00]); // QDCOUNT
        pkt.extend_from_slice(&[0x00, 0x01]); // ANCOUNT = 1
        pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // NS/AR counts

        // Answer name: same 0x20 + 32 bytes + 0x00 block as the query.
        pkt.push(0x20);
        pkt.extend_from_slice(&encode_netbios_name(b'*'));
        pkt.push(0x00);
        pkt.extend_from_slice(&QTYPE_NBSTAT.to_be_bytes());
        pkt.extend_from_slice(&QCLASS_IN.to_be_bytes());
        pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // TTL

        let mut rdata = vec![entries.len() as u8];
        for (name, suffix, group) in entries {
            let mut field = [b' '; 15];
            let bytes = name.as_bytes();
            field[..bytes.len()].copy_from_slice(bytes);
            rdata.extend_from_slice(&field);
            rdata.push(*suffix);
            let flags: u16 = if *group { 0x8400 } else { 0x0400 };
            rdata.extend_from_slice(&flags.to_be_bytes());
        }
        pkt.extend_from_slice(&(rdata.len() as u16).to_be_bytes()); // RDLENGTH
        pkt.extend_from_slice(&rdata);
        pkt
    }

    #[test]
    fn parses_names_and_picks_unique_workstation() {
        let response = build_response(&[
            ("MYPC", 0x00, false),     // unique workstation name
            ("WORKGROUP", 0x00, true), // domain/group name, must be skipped
            ("MYPC", 0x20, false),     // file server service
        ]);
        let names = parse_node_status_response(&response).unwrap();
        assert_eq!(names.len(), 3);
        assert_eq!(names[0].name, "MYPC");
        assert!(!names[0].group);
        assert!(names[1].group);
        assert_eq!(workstation_name(&names).as_deref(), Some("MYPC"));
    }

    #[test]
    fn trailing_padding_is_trimmed() {
        let response = build_response(&[("PC1", 0x00, false)]);
        let names = parse_node_status_response(&response).unwrap();
        assert_eq!(names[0].name, "PC1");
    }

    #[test]
    fn no_unique_workstation_name_yields_none() {
        let response = build_response(&[("WORKGROUP", 0x00, true)]);
        let names = parse_node_status_response(&response).unwrap();
        assert_eq!(workstation_name(&names), None);
    }

    #[test]
    fn truncated_messages_error_without_panicking() {
        assert!(parse_node_status_response(&[]).is_err());
        assert!(parse_node_status_response(&[0u8; 8]).is_err());
        // Header + name + RR fields claiming a name entry that is not present.
        let mut short = build_response(&[("PC", 0x00, false)]);
        short.truncate(short.len() - 5);
        assert!(parse_node_status_response(&short).is_err());
    }
}
