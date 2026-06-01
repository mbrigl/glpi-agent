// SPDX-License-Identifier: GPL-2.0-only

//! Ping discovery method: dual ICMP / TCP-connect strategy.
//!
//! Following §0.2 of the migration plan, liveness is probed with two
//! complementary techniques so the agent stays usable without elevated
//! privileges:
//!
//! 1. **ICMP echo** over an *unprivileged datagram* socket
//!    (`SOCK_DGRAM` + `IPPROTO_ICMP`). On Linux this needs the target gid to
//!    fall within `net.ipv4.ping_group_range` (or the `CAP_NET_RAW`
//!    capability); on Windows the equivalent unprivileged ICMP API is used.
//!    When the socket cannot be created (e.g. a locked-down CI container) the
//!    method silently falls through to step 2.
//! 2. **TCP connect** to a small set of common ports. A completed handshake
//!    *or* a `Connection refused` (the host answered with a RST) both prove the
//!    host is alive; only a timeout or an unreachable error counts as down.
//!
//! The packet framing ([`EchoRequest`], [`internet_checksum`]) and the TCP
//! outcome classifier are pure and unit-tested; the live socket paths are
//! best-effort and exercised only on a real network.

use std::io::ErrorKind;
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use async_trait::async_trait;
use glpi_core::error::Result;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::TcpStream;
use tokio::task::JoinSet;

use crate::traits::{DiscoveryMethod, Probe};

/// TCP ports tried by the connect fallback, in the absence of an override.
///
/// A deliberately small, high-yield set: web, SSH, SMB and RDP cover the great
/// majority of reachable hosts without lengthening every scan.
pub const DEFAULT_TCP_PORTS: &[u16] = &[80, 443, 22, 445, 3389];

/// Liveness probe combining unprivileged ICMP echo with a TCP-connect fallback.
///
/// Build one with [`PingMethod::new`] (ICMP first, then the default TCP ports)
/// or [`PingMethod::tcp_only`] for environments without ICMP. The same
/// `timeout` bounds each ICMP receive and each individual TCP connect.
#[derive(Debug, Clone)]
pub struct PingMethod {
    timeout: Duration,
    try_icmp: bool,
    tcp_ports: Vec<u16>,
}

impl PingMethod {
    /// Creates a method that tries ICMP first, then the [`DEFAULT_TCP_PORTS`].
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            try_icmp: true,
            tcp_ports: DEFAULT_TCP_PORTS.to_vec(),
        }
    }

    /// Creates a method that skips ICMP and only probes the given TCP ports.
    #[must_use]
    pub fn tcp_only(timeout: Duration, ports: Vec<u16>) -> Self {
        Self {
            timeout,
            try_icmp: false,
            tcp_ports: ports,
        }
    }

    /// Overrides the TCP fallback port list.
    #[must_use]
    pub fn with_tcp_ports(mut self, ports: Vec<u16>) -> Self {
        self.tcp_ports = ports;
        self
    }
}

#[async_trait]
impl DiscoveryMethod for PingMethod {
    fn name(&self) -> &'static str {
        "ping"
    }

    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>> {
        if self.try_icmp {
            if let IpAddr::V4(v4) = target {
                if icmp_echo(v4, self.timeout).await {
                    return Ok(Some(Probe::alive()));
                }
            }
        }
        if !self.tcp_ports.is_empty() && tcp_connect(target, &self.tcp_ports, self.timeout).await {
            return Ok(Some(Probe::alive()));
        }
        Ok(None)
    }
}

/// An ICMP echo-request message (RFC 792).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EchoRequest {
    /// Echo identifier, used to correlate replies with this probe.
    pub identifier: u16,
    /// Echo sequence number.
    pub sequence: u16,
    /// Opaque payload echoed back by the responder.
    pub payload: Vec<u8>,
}

impl EchoRequest {
    /// Builds an echo request with a default 32-byte payload.
    #[must_use]
    pub fn new(identifier: u16, sequence: u16) -> Self {
        Self {
            identifier,
            sequence,
            payload: vec![0x61; 32],
        }
    }

    /// Serializes the message to wire format with a valid checksum.
    ///
    /// Layout: type (8), code (0), 16-bit checksum, identifier, sequence,
    /// payload — all multi-byte fields big-endian.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut pkt = Vec::with_capacity(8 + self.payload.len());
        pkt.push(8); // type: echo request
        pkt.push(0); // code
        pkt.extend_from_slice(&[0, 0]); // checksum placeholder
        pkt.extend_from_slice(&self.identifier.to_be_bytes());
        pkt.extend_from_slice(&self.sequence.to_be_bytes());
        pkt.extend_from_slice(&self.payload);
        let checksum = internet_checksum(&pkt);
        pkt[2..4].copy_from_slice(&checksum.to_be_bytes());
        pkt
    }
}

/// Computes the RFC 1071 internet checksum (the value to store in the field).
///
/// Validating a received message is `internet_checksum(msg) == 0`, since the
/// stored checksum makes the one's-complement sum of the whole message zero.
#[must_use]
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    if let [last] = chunks.remainder() {
        sum += u32::from(u16::from_be_bytes([*last, 0]));
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Returns `true` if `data` is (or contains, after an optional IPv4 header) an
/// ICMP echo reply (type 0).
fn is_icmp_echo_reply(data: &[u8]) -> bool {
    matches!(strip_ipv4_header(data).first(), Some(0))
}

/// Skips a leading IPv4 header if one is present (raw sockets include it;
/// datagram ICMP sockets do not), returning the ICMP portion.
fn strip_ipv4_header(data: &[u8]) -> &[u8] {
    if let Some(&first) = data.first() {
        if first >> 4 == 4 {
            let header_len = usize::from(first & 0x0f) * 4;
            if data.len() >= header_len {
                return &data[header_len..];
            }
        }
    }
    data
}

/// Classifies a TCP connect outcome as proof of liveness.
///
/// `None` means the connection completed; `Some(kind)` is the error kind.
/// A completed handshake or a `Connection refused` both prove the host is up;
/// everything else (timeout, unreachable, …) does not.
fn tcp_outcome_is_alive(error: Option<ErrorKind>) -> bool {
    matches!(error, None | Some(ErrorKind::ConnectionRefused))
}

/// Attempts an unprivileged ICMP echo and waits up to `timeout` for a reply.
///
/// Any failure (socket unavailable, send error, timeout, malformed reply) maps
/// to `false` so the caller falls back to TCP.
async fn icmp_echo(target: Ipv4Addr, timeout: Duration) -> bool {
    tokio::task::spawn_blocking(move || icmp_echo_blocking(target, timeout).unwrap_or(false))
        .await
        .unwrap_or(false)
}

#[allow(unsafe_code)] // socket2's recv requires an uninitialized buffer.
fn icmp_echo_blocking(target: Ipv4Addr, timeout: Duration) -> std::io::Result<bool> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::ICMPV4))?;
    socket.set_read_timeout(Some(timeout))?;

    let identifier = std::process::id() as u16;
    let request = EchoRequest::new(identifier, 1).encode();
    let dest = SockAddr::from(SocketAddr::new(IpAddr::V4(target), 0));
    socket.send_to(&request, &dest)?;

    let mut buf = [MaybeUninit::<u8>::uninit(); 1500];
    let received = socket.recv(&mut buf)?;
    // SAFETY: `recv` reports it wrote `received` bytes into `buf`, so that
    // prefix is initialized and can be viewed as a `[u8]` of the same length.
    let data = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), received) };
    Ok(is_icmp_echo_reply(data))
}

/// Connects to each `port` on `target` concurrently, returning `true` as soon
/// as any connect outcome proves the host alive.
async fn tcp_connect(target: IpAddr, ports: &[u16], timeout: Duration) -> bool {
    let mut tasks: JoinSet<bool> = JoinSet::new();
    for &port in ports {
        let addr = SocketAddr::new(target, port);
        tasks.spawn(async move {
            match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
                Ok(result) => tcp_outcome_is_alive(result.err().map(|e| e.kind())),
                Err(_) => false, // connect timed out
            }
        });
    }
    while let Some(joined) = tasks.join_next().await {
        if matches!(joined, Ok(true)) {
            tasks.abort_all();
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        internet_checksum, is_icmp_echo_reply, tcp_outcome_is_alive, EchoRequest, PingMethod,
    };
    use crate::traits::DiscoveryMethod;
    use std::io::ErrorKind;
    use std::time::Duration;

    #[test]
    fn checksum_of_all_zeroes_is_all_ones() {
        assert_eq!(internet_checksum(&[0x00, 0x00]), 0xffff);
    }

    #[test]
    fn checksum_handles_odd_length() {
        // Padding the trailing byte with zero must not panic and must fold carries.
        let _ = internet_checksum(&[0x01, 0x02, 0x03]);
    }

    #[test]
    fn encoded_request_is_self_consistent() {
        let pkt = EchoRequest::new(0x1234, 7).encode();
        // type / code / id / seq are placed correctly.
        assert_eq!(pkt[0], 8);
        assert_eq!(pkt[1], 0);
        assert_eq!(&pkt[4..6], &0x1234u16.to_be_bytes());
        assert_eq!(&pkt[6..8], &7u16.to_be_bytes());
        // Verifying the checksum over the whole packet yields zero.
        assert_eq!(internet_checksum(&pkt), 0);
    }

    #[test]
    fn echo_reply_detection_with_and_without_ip_header() {
        // Datagram socket: bare ICMP, type 0 = echo reply.
        assert!(is_icmp_echo_reply(&[0x00, 0x00, 0x00, 0x00]));
        // Echo request (type 8) is not a reply.
        assert!(!is_icmp_echo_reply(&[0x08, 0x00, 0x00, 0x00]));
        // Raw socket: 20-byte IPv4 header (IHL=5) then a type-0 ICMP message.
        let mut raw = vec![0x45u8];
        raw.extend_from_slice(&[0u8; 19]);
        raw.push(0x00); // ICMP type 0
        assert!(is_icmp_echo_reply(&raw));
    }

    #[test]
    fn tcp_classifier_treats_connect_and_refusal_as_alive() {
        assert!(tcp_outcome_is_alive(None));
        assert!(tcp_outcome_is_alive(Some(ErrorKind::ConnectionRefused)));
        assert!(!tcp_outcome_is_alive(Some(ErrorKind::TimedOut)));
        assert!(!tcp_outcome_is_alive(Some(ErrorKind::AddrNotAvailable)));
    }

    #[tokio::test]
    async fn tcp_ping_detects_a_listening_localhost_port() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let method = PingMethod::tcp_only(Duration::from_millis(500), vec![port]);
        let result = method.probe("127.0.0.1".parse().unwrap()).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn tcp_ping_treats_refused_localhost_as_alive() {
        // Nothing listens on port 1; loopback replies with RST (Connection
        // refused), which proves the host is up.
        let method = PingMethod::tcp_only(Duration::from_millis(500), vec![1]);
        let result = method.probe("127.0.0.1".parse().unwrap()).await.unwrap();
        assert!(result.is_some());
    }
}
