// SPDX-License-Identifier: GPL-2.0-only

//! The inventory `content` object.
//!
//! [`Content`] is the typed payload of a local inventory — the concrete
//! per-category data that `glpi-core`'s generic `InventoryRequest<C>` carries.
//! Field names serialize to the GLPI native JSON keys. It grows one optional
//! section per category as they are implemented.

use serde::Serialize;

use crate::categories::{
    Battery, Bios, Controller, Cpu, EnvVar, Hardware, MemoryModule, NetworkInterface,
    OperatingSystem, Process, Software, Sound, Storage, UsbDevice, User, Video,
};

/// The assembled local-inventory content.
///
/// Empty sections are omitted from serialization so a partial inventory only
/// carries what was actually collected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Content {
    /// BIOS / system / motherboard identity.
    #[serde(rename = "bios", skip_serializing_if = "Option::is_none")]
    pub bios: Option<Bios>,
    /// Device-level identity (hostname, UUID).
    #[serde(rename = "hardware", skip_serializing_if = "Option::is_none")]
    pub hardware: Option<Hardware>,
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
    /// Storage devices.
    #[serde(rename = "storages", skip_serializing_if = "Vec::is_empty")]
    pub storages: Vec<Storage>,
    /// Running processes.
    #[serde(rename = "processes", skip_serializing_if = "Vec::is_empty")]
    pub processes: Vec<Process>,
    /// PCI controllers.
    #[serde(rename = "controllers", skip_serializing_if = "Vec::is_empty")]
    pub controllers: Vec<Controller>,
    /// USB devices.
    #[serde(rename = "usbdevices", skip_serializing_if = "Vec::is_empty")]
    pub usb_devices: Vec<UsbDevice>,
    /// Logged-in users.
    #[serde(rename = "users", skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<User>,
    /// Batteries.
    #[serde(rename = "batteries", skip_serializing_if = "Vec::is_empty")]
    pub batteries: Vec<Battery>,
    /// Process environment variables.
    #[serde(rename = "envs", skip_serializing_if = "Vec::is_empty")]
    pub envs: Vec<EnvVar>,
    /// Video controllers.
    #[serde(rename = "videos", skip_serializing_if = "Vec::is_empty")]
    pub videos: Vec<Video>,
    /// Sound cards.
    #[serde(rename = "sounds", skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<Sound>,
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
