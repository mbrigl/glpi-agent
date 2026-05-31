// SPDX-License-Identifier: GPL-2.0-only

//! SNMP credential types used by the discovery and network-inventory tasks.
//!
//! The full SNMPv3 USM crypto matrix is implemented later in `glpi-discovery`;
//! these types only model the *configuration* of a credential set.

use serde::{Deserialize, Serialize};

/// SNMP protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SnmpVersion {
    /// SNMPv1 (community based).
    #[serde(rename = "1")]
    V1,
    /// SNMPv2c (community based).
    #[default]
    #[serde(rename = "2c")]
    V2c,
    /// SNMPv3 (User-based Security Model).
    #[serde(rename = "3")]
    V3,
}

/// SNMPv3 USM authentication algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuthProtocol {
    /// HMAC-MD5-96.
    Md5,
    /// HMAC-SHA-96.
    Sha1,
    /// HMAC-SHA-224.
    Sha224,
    /// HMAC-SHA-256.
    Sha256,
    /// HMAC-SHA-384.
    Sha384,
    /// HMAC-SHA-512.
    Sha512,
}

/// SNMPv3 USM privacy (encryption) algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PrivProtocol {
    /// CBC-DES.
    Des,
    /// CFB128-AES-128.
    Aes128,
    /// CFB128-AES-192.
    Aes192,
    /// CFB128-AES-256.
    Aes256,
    /// CFB128-AES-192 with the Cisco key-extension ("AES-192-C").
    Aes192c,
    /// CFB128-AES-256 with the Cisco key-extension ("AES-256-C").
    Aes256c,
}

/// A complete SNMP credential set covering v1/v2c/v3.
///
/// For v1/v2c only [`community`](Self::community) is relevant; for v3 the USM
/// fields apply. Field presence is validated by the discovery task, not here.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpCredentials {
    /// Protocol version this credential set is for.
    pub version: SnmpVersion,
    /// Community string (v1/v2c).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community: Option<String>,
    /// USM user name (v3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Authentication algorithm (v3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_protocol: Option<AuthProtocol>,
    /// Authentication passphrase (v3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_password: Option<String>,
    /// Privacy algorithm (v3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priv_protocol: Option<PrivProtocol>,
    /// Privacy passphrase (v3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priv_password: Option<String>,
    /// SNMPv3 context name (added upstream in agent 1.17).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_name: Option<String>,
}

impl SnmpCredentials {
    /// Builds a community-based v2c credential set.
    #[must_use]
    pub fn v2c(community: impl Into<String>) -> Self {
        Self {
            version: SnmpVersion::V2c,
            community: Some(community.into()),
            ..Self::default()
        }
    }

    /// Returns `true` if this credential set requires the USM crypto stack.
    #[must_use]
    pub fn is_v3(&self) -> bool {
        self.version == SnmpVersion::V3
    }
}

#[cfg(test)]
mod tests {
    use super::{SnmpCredentials, SnmpVersion};

    #[test]
    fn v2c_helper_sets_fields() {
        let creds = SnmpCredentials::v2c("public");
        assert_eq!(creds.version, SnmpVersion::V2c);
        assert_eq!(creds.community.as_deref(), Some("public"));
        assert!(!creds.is_v3());
    }

    #[test]
    fn version_serializes_to_short_string() {
        assert_eq!(serde_json::to_string(&SnmpVersion::V2c).unwrap(), "\"2c\"");
        assert_eq!(serde_json::to_string(&SnmpVersion::V3).unwrap(), "\"3\"");
    }

    #[test]
    fn default_version_is_v2c() {
        assert_eq!(SnmpVersion::default(), SnmpVersion::V2c);
    }
}
