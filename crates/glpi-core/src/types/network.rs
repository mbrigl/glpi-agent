// SPDX-License-Identifier: GPL-2.0-only

//! Network-related value types: [`MacAddress`] and [`NetworkInterface`].

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::AgentError;

/// A 48-bit IEEE 802 MAC address.
///
/// Parsing accepts the common colon- and hyphen-separated hexadecimal forms
/// (`00:1a:2b:3c:4d:5e` or `00-1A-2B-3C-4D-5E`); the canonical [`Display`]
/// form is lower-case, colon-separated. The type (de)serializes as that
/// string so it round-trips through the GLPI JSON protocol unchanged.
///
/// [`Display`]: std::fmt::Display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    /// Builds a MAC address from its six raw octets.
    #[must_use]
    pub const fn new(octets: [u8; 6]) -> Self {
        Self(octets)
    }

    /// Returns the six octets in network order.
    #[must_use]
    pub const fn octets(&self) -> [u8; 6] {
        self.0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02x}:{b:02x}:{c:02x}:{d:02x}:{e:02x}:{g:02x}")
    }
}

impl FromStr for MacAddress {
    type Err = AgentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut octets = [0u8; 6];
        let mut count = 0usize;
        for part in s.split([':', '-']) {
            if count == 6 {
                count += 1;
                break;
            }
            octets[count] = u8::from_str_radix(part, 16)
                .map_err(|_| AgentError::Parse(format!("invalid MAC address: {s}")))?;
            count += 1;
        }
        if count != 6 {
            return Err(AgentError::Parse(format!("invalid MAC address: {s}")));
        }
        Ok(Self(octets))
    }
}

impl TryFrom<String> for MacAddress {
    type Error = AgentError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<MacAddress> for String {
    fn from(value: MacAddress) -> Self {
        value.to_string()
    }
}

/// A single network interface as reported by an inventory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    /// Human-readable interface name (for example `eth0` or `Ethernet 2`).
    pub description: String,
    /// Hardware (MAC) address, if the interface has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<MacAddress>,
    /// IPv4 and IPv6 addresses currently bound to the interface.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ip_addresses: Vec<IpAddr>,
    /// Link speed in megabits per second, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mbps: Option<u64>,
    /// Whether the interface is administratively up.
    pub up: bool,
}

#[cfg(test)]
mod tests {
    use super::MacAddress;

    #[test]
    fn parses_colon_and_hyphen_forms() {
        let a: MacAddress = "00:1a:2b:3c:4d:5e".parse().unwrap();
        let b: MacAddress = "00-1A-2B-3C-4D-5E".parse().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.octets(), [0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]);
    }

    #[test]
    fn display_is_lowercase_colon_form() {
        let mac = MacAddress::new([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]);
        assert_eq!(mac.to_string(), "00:1a:2b:3c:4d:5e");
    }

    #[test]
    fn rejects_wrong_length() {
        assert!("00:11:22".parse::<MacAddress>().is_err());
        assert!("00:11:22:33:44:55:66".parse::<MacAddress>().is_err());
    }

    #[test]
    fn json_round_trip() {
        let mac = MacAddress::new([0xde, 0xad, 0xbe, 0xef, 0x00, 0x01]);
        let json = serde_json::to_string(&mac).unwrap();
        assert_eq!(json, "\"de:ad:be:ef:00:01\"");
        let back: MacAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(mac, back);
    }
}
