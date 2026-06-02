// SPDX-License-Identifier: GPL-2.0-only

//! The Wake-on-LAN task: broadcast a magic packet for each target MAC.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use glpi_core::error::{AgentError, Result};
use glpi_core::types::network::MacAddress;

use crate::magic_packet::MagicPacket;

/// The UDP ports a magic packet is conventionally broadcast to.
pub const DEFAULT_PORTS: &[u16] = &[9, 7];

/// A Wake-on-LAN task: a set of target MACs and the broadcast configuration.
#[derive(Debug, Clone)]
pub struct WakeOnLanTask {
    macs: Vec<MacAddress>,
    password: Option<Vec<u8>>,
    ports: Vec<u16>,
}

impl WakeOnLanTask {
    /// Builds a task that wakes every MAC in `macs` on the [`DEFAULT_PORTS`].
    #[must_use]
    pub fn new(macs: Vec<MacAddress>) -> Self {
        Self {
            macs,
            password: None,
            ports: DEFAULT_PORTS.to_vec(),
        }
    }

    /// Sets a SecureOn password appended to each packet.
    #[must_use]
    pub fn with_password(mut self, password: Vec<u8>) -> Self {
        self.password = Some(password);
        self
    }

    /// Overrides the UDP ports the packets are sent to.
    #[must_use]
    pub fn with_ports(mut self, ports: Vec<u16>) -> Self {
        self.ports = ports;
        self
    }

    /// Broadcasts a magic packet for every (MAC, port) pair to the limited
    /// broadcast address `255.255.255.255`.
    ///
    /// Returns the number of packets sent.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Io`] if the broadcast socket cannot be opened, or
    /// [`AgentError::Task`] if every send fails.
    pub fn wake(&self) -> Result<usize> {
        let socket = open_broadcast_socket()?;
        let mut sent = 0;
        let mut last_error = None;
        for mac in &self.macs {
            let packet = match &self.password {
                Some(password) => MagicPacket::with_password(*mac, password),
                None => MagicPacket::new(*mac),
            };
            for &port in &self.ports {
                let addr = SocketAddr::from((Ipv4Addr::BROADCAST, port));
                match send(&socket, packet.as_bytes(), addr) {
                    Ok(()) => sent += 1,
                    Err(err) => {
                        tracing::warn!(%mac, port, error = %err, "wake-on-lan send failed");
                        last_error = Some(err);
                    }
                }
            }
        }
        if sent == 0 {
            if let Some(err) = last_error {
                return Err(AgentError::Task(format!(
                    "all wake-on-lan sends failed: {err}"
                )));
            }
        }
        tracing::info!(
            targets = self.macs.len(),
            packets = sent,
            "wake-on-lan packets broadcast"
        );
        Ok(sent)
    }
}

/// Opens an ephemeral UDP socket with broadcast enabled.
fn open_broadcast_socket() -> Result<UdpSocket> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(true)?;
    Ok(socket)
}

/// Sends `bytes` to `addr` on `socket`.
fn send(socket: &UdpSocket, bytes: &[u8], addr: SocketAddr) -> std::io::Result<()> {
    socket.send_to(bytes, addr).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{send, WakeOnLanTask, DEFAULT_PORTS};
    use crate::magic_packet::{MagicPacket, MAGIC_PACKET_LEN};
    use glpi_core::types::network::MacAddress;
    use std::net::{Ipv4Addr, UdpSocket};

    #[test]
    fn default_ports_are_9_and_7() {
        assert_eq!(DEFAULT_PORTS, &[9, 7]);
    }

    #[test]
    fn wake_with_no_targets_sends_nothing() {
        let task = WakeOnLanTask::new(Vec::new());
        assert_eq!(task.wake().unwrap(), 0);
    }

    #[test]
    fn send_delivers_the_magic_packet_over_udp() {
        // Bind a receiver on loopback and send the packet to it directly
        // (unicast), proving the on-wire bytes match the magic packet.
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = receiver.local_addr().unwrap();
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();

        let mac = MacAddress::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let packet = MagicPacket::new(mac);
        send(&sender, packet.as_bytes(), addr).unwrap();

        let mut buf = [0u8; 256];
        let (len, _) = receiver.recv_from(&mut buf).unwrap();
        assert_eq!(len, MAGIC_PACKET_LEN);
        assert_eq!(&buf[..len], packet.as_bytes());
    }
}
