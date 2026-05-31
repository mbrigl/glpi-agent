// SPDX-License-Identifier: GPL-2.0-only

//! Device identity types: [`AssetType`] and [`Device`].

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::AgentError;

/// The GLPI asset type (`itemtype`) a discovered or inventoried device maps to.
///
/// GLPI 11 generalized assets, so any string is a valid itemtype. The common
/// built-in types are modelled as variants for ergonomics; anything else is
/// preserved verbatim in [`AssetType::Other`]. The type (de)serializes as the
/// GLPI itemtype string.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum AssetType {
    /// `Computer`.
    Computer,
    /// `NetworkEquipment` (switch, router, firewall…).
    NetworkEquipment,
    /// `Printer`.
    Printer,
    /// `Phone`.
    Phone,
    /// `Peripheral`.
    Peripheral,
    /// `Monitor`.
    Monitor,
    /// `Unmanaged` (discovered but not yet classified).
    #[default]
    Unmanaged,
    /// Any other GLPI itemtype, kept as-is.
    Other(String),
}

impl AssetType {
    /// Returns the GLPI itemtype string for this asset type.
    #[must_use]
    pub fn as_itemtype(&self) -> &str {
        match self {
            Self::Computer => "Computer",
            Self::NetworkEquipment => "NetworkEquipment",
            Self::Printer => "Printer",
            Self::Phone => "Phone",
            Self::Peripheral => "Peripheral",
            Self::Monitor => "Monitor",
            Self::Unmanaged => "Unmanaged",
            Self::Other(s) => s,
        }
    }
}

impl fmt::Display for AssetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_itemtype())
    }
}

impl FromStr for AssetType {
    type Err = AgentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Computer" => Self::Computer,
            "NetworkEquipment" => Self::NetworkEquipment,
            "Printer" => Self::Printer,
            "Phone" => Self::Phone,
            "Peripheral" => Self::Peripheral,
            "Monitor" => Self::Monitor,
            "Unmanaged" => Self::Unmanaged,
            "" => return Err(AgentError::Parse("empty itemtype".to_owned())),
            other => Self::Other(other.to_owned()),
        })
    }
}

impl TryFrom<String> for AssetType {
    type Error = AgentError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<AssetType> for String {
    fn from(value: AssetType) -> Self {
        value.to_string()
    }
}

/// The identifying attributes of a device, independent of its inventory detail.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Display name / hostname.
    pub name: String,
    /// GLPI asset type this device maps to.
    pub asset_type: AssetType,
    /// SMBIOS / hardware UUID, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// Manufacturer / vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Model name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::AssetType;

    #[test]
    fn known_types_round_trip_as_itemtype() {
        let t = AssetType::NetworkEquipment;
        assert_eq!(t.as_itemtype(), "NetworkEquipment");
        assert_eq!("NetworkEquipment".parse::<AssetType>().unwrap(), t);
    }

    #[test]
    fn unknown_type_is_preserved() {
        let t: AssetType = "Datacenter".parse().unwrap();
        assert_eq!(t, AssetType::Other("Datacenter".to_owned()));
        assert_eq!(t.as_itemtype(), "Datacenter");
    }

    #[test]
    fn empty_itemtype_is_rejected() {
        assert!("".parse::<AssetType>().is_err());
    }

    #[test]
    fn json_uses_itemtype_string() {
        let json = serde_json::to_string(&AssetType::Computer).unwrap();
        assert_eq!(json, "\"Computer\"");
    }
}
