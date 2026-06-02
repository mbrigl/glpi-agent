// SPDX-License-Identifier: GPL-2.0-only

//! IEC 61850 device scan and inventory.
//!
//! Ported from the upstream `GLPI::Agent::IEC61850::{Protocol,Device}`: walk the
//! first server logical device, find its `LPHD<n>` physical-device node, read
//! the `PhyNam` (physical nameplate) attributes, and assemble the GLPI
//! inventory (INFO / ITEMTYPE / FIRMWARES). The traversal runs over the
//! [`IedProtocol`] seam, so it is exercised end-to-end against a mock IED.

use serde::Serialize;

use glpi_core::error::Result;

use crate::protocol::{FunctionalConstraint, IedProtocol};

/// `PhyNam` attributes read from the physical-device node, in upstream order.
const PHYNAM_VARIABLES: [&str; 7] = [
    "model", "hwRev", "vendor", "serNum", "swRev", "owner", "location",
];

/// The GLPI `itemtype` for an IED on GLPI 11+ (custom asset).
const IED_ITEMTYPE: &str = r"Glpi\CustomAsset\IedAsset";

/// Identity collected from an IED's physical nameplate (`PhyNam`) plus its
/// logical-device name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IedIdentity {
    /// Logical-device name (the IED name).
    pub ied_name: Option<String>,
    /// `PhyNam.vendor`.
    pub manufacturer: Option<String>,
    /// `PhyNam.model`.
    pub model: Option<String>,
    /// `PhyNam.serNum`.
    pub serial: Option<String>,
    /// `PhyNam.swRev` (software / firmware revision).
    pub firmware: Option<String>,
    /// `PhyNam.hwRev` (hardware revision).
    pub hardware: Option<String>,
    /// `PhyNam.owner`.
    pub contact: Option<String>,
    /// `PhyNam.location`.
    pub location: Option<String>,
}

impl IedIdentity {
    /// Scans `protocol` for the first IED's identity (physical nameplate).
    ///
    /// Mirrors the upstream traversal: the first logical device, its first
    /// `LPHD<n>` node, then that node's `PhyNam` attributes.
    ///
    /// # Errors
    ///
    /// Propagates a transport/protocol failure from `protocol`.
    pub async fn scan<P: IedProtocol + ?Sized>(protocol: &mut P) -> Result<Self> {
        let mut identity = Self::default();

        let Some(device) = protocol.server_directory().await?.into_iter().next() else {
            return Ok(identity);
        };
        identity.ied_name = Some(device.clone());

        let nodes = protocol.logical_device_directory(&device).await?;
        let Some(lphd) = nodes.iter().find(|node| is_physical_device_node(node)) else {
            return Ok(identity);
        };
        let node_ref = format!("{device}/{lphd}");

        let objects = protocol.logical_node_directory(&node_ref).await?;
        if !objects.iter().any(|object| object == "PhyNam") {
            return Ok(identity);
        }

        for variable in PHYNAM_VARIABLES {
            let reference = format!("{node_ref}.PhyNam.{variable}");
            let value = protocol
                .read_string(&reference, FunctionalConstraint::DC)
                .await?
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty());
            let Some(value) = value else { continue };
            match variable {
                "model" => identity.model = Some(value),
                "hwRev" => identity.hardware = Some(value),
                "vendor" => identity.manufacturer = Some(value),
                "serNum" => identity.serial = Some(value),
                "swRev" => identity.firmware = Some(value),
                "owner" => identity.contact = Some(value),
                "location" => identity.location = Some(value),
                _ => {}
            }
        }
        Ok(identity)
    }

    /// Assembles the GLPI inventory for this IED.
    ///
    /// `glpi_version` (the target server version) selects the GLPI 11+ IED
    /// itemtype; `mac` / `ip` are folded into the `INFO` section when known.
    #[must_use]
    pub fn into_inventory(
        self,
        glpi_version: Option<&str>,
        mac: Option<String>,
        ip: Option<String>,
    ) -> IedInventory {
        let model_label = self.model.clone();
        let manufacturer = self.manufacturer.clone();

        // Firmware (always) and hardware (when present) entries.
        let device_label = |kind: &str| {
            format!(
                "{} {kind}",
                model_label.as_deref().unwrap_or("Electronic device")
            )
        };
        let mut firmwares = Vec::new();
        if self.firmware.is_some() {
            firmwares.push(IedFirmware {
                name: device_label("firmware"),
                description: "Electronic device firmware".to_owned(),
                r#type: "ied".to_owned(),
                version: self.firmware.clone(),
                manufacturer: manufacturer.clone(),
            });
        }
        if let Some(hardware) = &self.hardware {
            firmwares.push(IedFirmware {
                name: device_label("hardware"),
                description: "Electronic device hardware".to_owned(),
                r#type: "ied".to_owned(),
                version: Some(hardware.clone()),
                manufacturer: manufacturer.clone(),
            });
        }

        // The IED name loses manufacturer-specific suffixes (e.g. Siemens
        // logical-device names carry a trailing `A_Allg`).
        let name = self
            .ied_name
            .map(|name| name.strip_suffix("A_Allg").unwrap_or(&name).to_owned());

        let info = IedInfo {
            r#type: "NETWORKING".to_owned(),
            manufacturer,
            model: self.model,
            serial: self.serial,
            firmware: self.firmware,
            contact: self.contact,
            location: self.location,
            name,
            mac,
            ips: ip.map(|ip| IedIps { ip }),
        };

        IedInventory {
            itemtype: supports_ied_asset(glpi_version).then(|| IED_ITEMTYPE.to_owned()),
            info,
            firmwares,
        }
    }
}

/// `true` for a physical-device logical node (`LPHD` followed by digits).
fn is_physical_device_node(name: &str) -> bool {
    name.strip_prefix("LPHD")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// `true` if `glpi_version`'s major component is 11 or newer.
fn supports_ied_asset(glpi_version: Option<&str>) -> bool {
    glpi_version
        .and_then(|v| v.split('.').next())
        .and_then(|major| major.parse::<u32>().ok())
        .is_some_and(|major| major >= 11)
}

/// A firmware entry of an IED inventory (GLPI `FIRMWARES`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IedFirmware {
    /// Entry name (`<model> firmware` / `<model> hardware`).
    #[serde(rename = "NAME")]
    pub name: String,
    /// Human-readable description.
    #[serde(rename = "DESCRIPTION")]
    pub description: String,
    /// Entry kind (always `ied`).
    #[serde(rename = "TYPE")]
    pub r#type: String,
    /// Version string.
    #[serde(rename = "VERSION", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Manufacturer.
    #[serde(rename = "MANUFACTURER", skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
}

/// The `IPS` sub-object of an IED `INFO` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IedIps {
    /// The device IP address.
    #[serde(rename = "IP")]
    pub ip: String,
}

/// The `INFO` section of an IED inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IedInfo {
    /// Asset type (always `NETWORKING`).
    #[serde(rename = "TYPE")]
    pub r#type: String,
    /// Manufacturer (`PhyNam.vendor`).
    #[serde(rename = "MANUFACTURER", skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Model (`PhyNam.model`).
    #[serde(rename = "MODEL", skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Serial number (`PhyNam.serNum`).
    #[serde(rename = "SERIAL", skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// Firmware revision (`PhyNam.swRev`).
    #[serde(rename = "FIRMWARE", skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    /// Administrative contact (`PhyNam.owner`).
    #[serde(rename = "CONTACT", skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// Physical location (`PhyNam.location`).
    #[serde(rename = "LOCATION", skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// IED name (cleaned logical-device name).
    #[serde(rename = "NAME", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// MAC address, when known from discovery.
    #[serde(rename = "MAC", skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    /// IP address, when known from discovery.
    #[serde(rename = "IPS", skip_serializing_if = "Option::is_none")]
    pub ips: Option<IedIps>,
}

/// A complete IED inventory (`INFO` + optional `ITEMTYPE` + `FIRMWARES`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IedInventory {
    /// The device identity / attributes.
    #[serde(rename = "INFO")]
    pub info: IedInfo,
    /// GLPI 11+ custom-asset itemtype, when targeting a recent server.
    #[serde(rename = "ITEMTYPE", skip_serializing_if = "Option::is_none")]
    pub itemtype: Option<String>,
    /// Firmware / hardware entries.
    #[serde(rename = "FIRMWARES")]
    pub firmwares: Vec<IedFirmware>,
}

#[cfg(test)]
mod tests {
    use super::{is_physical_device_node, supports_ied_asset};

    #[test]
    fn detects_physical_device_node() {
        assert!(is_physical_device_node("LPHD1"));
        assert!(is_physical_device_node("LPHD12"));
        assert!(!is_physical_device_node("LPHD"));
        assert!(!is_physical_device_node("LLN0"));
        assert!(!is_physical_device_node("LPHDx"));
    }

    #[test]
    fn ied_asset_needs_glpi_11() {
        assert!(supports_ied_asset(Some("11")));
        assert!(supports_ied_asset(Some("11.0.1")));
        assert!(supports_ied_asset(Some("12.3")));
        assert!(!supports_ied_asset(Some("10.0.19")));
        assert!(!supports_ied_asset(None));
        assert!(!supports_ied_asset(Some("")));
    }
}
