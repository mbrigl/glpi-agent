// SPDX-License-Identifier: GPL-2.0-only

//! SNMP support, built on the [`snmp2`] crate.
//!
//! `snmp2` supplies the wire codec and the full v1/v2c/v3 client (including the
//! complete USM auth/priv matrix and the AES key-localization methods); this
//! module adapts it to the agent's types. Landing incrementally; currently
//! available:
//!
//! - [`value`] — [`SnmpValue`], an owned projection of `snmp2::Value`,
//! - [`credentials`] — mapping [`SnmpCredentials`] onto `snmp2`'s community
//!   string and v3 [`Security`].
//!
//! The async client wrapper and the SNMP [`DiscoveryMethod`] follow in the next
//! units.
//!
//! [`SnmpCredentials`]: glpi_core::types::snmp::SnmpCredentials
//! [`Security`]: snmp2::v3::Security
//! [`DiscoveryMethod`]: crate::traits::DiscoveryMethod

pub mod credentials;
pub mod value;

pub use credentials::{
    build_security, community, map_auth_protocol, map_priv_cipher, priv_key_extension,
    security_level, SecurityLevel,
};
pub use value::SnmpValue;
