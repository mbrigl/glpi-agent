// SPDX-License-Identifier: GPL-2.0-only

//! The inventory `content` object.
//!
//! [`Content`] is the typed payload of a local inventory — the concrete
//! per-category data that `glpi-core`'s generic `InventoryRequest<C>` carries.
//! Field names serialize to the GLPI native JSON keys. It grows one optional
//! section per category as they are implemented.

use serde::Serialize;

use crate::categories::{Cpu, MemoryModule, NetworkInterface, OperatingSystem, Software};

/// The assembled local-inventory content.
///
/// Empty sections are omitted from serialization so a partial inventory only
/// carries what was actually collected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Content {
    /// Operating-system identity.
    #[serde(rename = "operatingsystem", skip_serializing_if = "Option::is_none")]
    pub operating_system: Option<OperatingSystem>,
    /// Physical CPUs.
    #[serde(rename = "cpus", skip_serializing_if = "Vec::is_empty")]
    pub cpus: Vec<Cpu>,
    /// Memory modules.
    #[serde(rename = "memories", skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<MemoryModule>,
    /// Installed software packages.
    #[serde(rename = "softwares", skip_serializing_if = "Vec::is_empty")]
    pub softwares: Vec<Software>,
    /// Network interfaces.
    #[serde(rename = "networks", skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<NetworkInterface>,
}

#[cfg(test)]
mod tests {
    use super::Content;
    use crate::categories::parse_os_release;

    #[test]
    fn empty_content_serializes_to_empty_object() {
        assert_eq!(serde_json::to_string(&Content::default()).unwrap(), "{}");
    }

    #[test]
    fn operating_system_uses_the_glpi_key() {
        let content = Content {
            operating_system: Some(parse_os_release("NAME=\"Ubuntu\"\nVERSION_ID=\"22.04\"\n")),
            ..Content::default()
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["operatingsystem"]["name"], "Ubuntu");
        assert_eq!(json["operatingsystem"]["version"], "22.04");
    }
}
