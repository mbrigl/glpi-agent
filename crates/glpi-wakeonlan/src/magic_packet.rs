// SPDX-License-Identifier: GPL-2.0-only

//! The Wake-on-LAN magic packet.
//!
//! A magic packet is six `0xFF` bytes followed by the target MAC repeated 16
//! times — 102 bytes. An optional 4- or 6-byte SecureOn password may be
//! appended, giving a 106- or 108-byte packet.

use glpi_core::types::network::MacAddress;

/// The fixed magic-packet length without a SecureOn password.
pub const MAGIC_PACKET_LEN: usize = 102;

/// A serialized Wake-on-LAN magic packet for a single target MAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicPacket {
    bytes: Vec<u8>,
}

impl MagicPacket {
    /// Builds the 102-byte magic packet for `mac`.
    #[must_use]
    pub fn new(mac: MacAddress) -> Self {
        Self::build(mac, &[])
    }

    /// Builds a magic packet for `mac` with a SecureOn `password` appended.
    ///
    /// The password is appended verbatim; SecureOn uses 4 or 6 bytes, so the
    /// resulting packet is 106 or 108 bytes. Any length is accepted — the
    /// caller is responsible for using a valid SecureOn length.
    #[must_use]
    pub fn with_password(mac: MacAddress, password: &[u8]) -> Self {
        Self::build(mac, password)
    }

    /// Assembles the synchronization stream, the 16 MAC repetitions and the
    /// optional password suffix.
    fn build(mac: MacAddress, password: &[u8]) -> Self {
        let octets = mac.octets();
        let mut bytes = Vec::with_capacity(MAGIC_PACKET_LEN + password.len());
        bytes.extend_from_slice(&[0xFF; 6]);
        for _ in 0..16 {
            bytes.extend_from_slice(&octets);
        }
        bytes.extend_from_slice(password);
        Self { bytes }
    }

    /// Returns the packet bytes ready to send over UDP.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{MagicPacket, MAGIC_PACKET_LEN};
    use glpi_core::types::network::MacAddress;

    fn mac() -> MacAddress {
        MacAddress::new([0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01])
    }

    #[test]
    fn packet_is_102_bytes() {
        let packet = MagicPacket::new(mac());
        assert_eq!(packet.as_bytes().len(), MAGIC_PACKET_LEN);
    }

    #[test]
    fn packet_starts_with_six_ff_then_repeats_the_mac() {
        let packet = MagicPacket::new(mac());
        let bytes = packet.as_bytes();
        assert_eq!(&bytes[..6], &[0xFF; 6]);
        // Every following 6-byte chunk is the target MAC, 16 times.
        for chunk in bytes[6..].chunks(6) {
            assert_eq!(chunk, &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);
        }
        assert_eq!(bytes[6..].chunks(6).count(), 16);
    }

    #[test]
    fn secureon_password_is_appended() {
        let packet = MagicPacket::with_password(mac(), &[1, 2, 3, 4, 5, 6]);
        assert_eq!(packet.as_bytes().len(), MAGIC_PACKET_LEN + 6);
        assert_eq!(&packet.as_bytes()[MAGIC_PACKET_LEN..], &[1, 2, 3, 4, 5, 6]);
    }
}
