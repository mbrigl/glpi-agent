// SPDX-License-Identifier: GPL-2.0-only

//! The NetInventory result types that MIB modules populate.
//!
//! A [`NetworkDevice`] is built up by running [`MibSupport`] modules against a
//! device: the standard MIBs fill the base [`DeviceInfo`] and the port/component
//! tables, and vendor MIBs refine them. The structure mirrors the GLPI network
//! device schema (INFO / PORTS / COMPONENTS) and grows as more MIBs land.
//!
//! [`MibSupport`]: super::MibSupport

use glpi_core::types::network::MacAddress;

/// Base identity and attributes of a discovered network device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    /// `sysDescr` — free-form description (vendor/model/firmware text).
    pub description: Option<String>,
    /// `sysName` — configured node name.
    pub name: Option<String>,
    /// `sysContact` — administrative contact.
    pub contact: Option<String>,
    /// `sysLocation` — physical location.
    pub location: Option<String>,
    /// `sysUpTime` in timeticks (hundredths of a second).
    pub uptime: Option<u32>,
    /// `sysObjectID` in dotted form.
    pub sys_object_id: Option<String>,
    /// Manufacturer, from `sysobject.ids` classification or a vendor MIB.
    pub manufacturer: Option<String>,
    /// Device type (`NETWORKING`, `PRINTER`, …).
    pub r#type: Option<String>,
    /// Model name.
    pub model: Option<String>,
}

/// A network interface (a row of `ifTable` / `ifXTable`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Port {
    /// `ifIndex` — the interface's table index.
    pub index: u64,
    /// `ifName` (from `ifXTable`), the short interface name.
    pub name: Option<String>,
    /// `ifDescr`, the interface description.
    pub description: Option<String>,
    /// `ifAlias` (from `ifXTable`), an operator-assigned label.
    pub alias: Option<String>,
    /// `ifType` (IANAifType numeric value).
    pub if_type: Option<i64>,
    /// `ifMtu`.
    pub mtu: Option<i64>,
    /// Interface speed in bits per second (`ifSpeed`, or `ifHighSpeed`×1e6).
    pub speed: Option<u64>,
    /// `ifPhysAddress` — the interface MAC, when a valid one is present.
    pub mac: Option<MacAddress>,
    /// `ifAdminStatus` (1 = up, 2 = down, 3 = testing).
    pub admin_status: Option<i64>,
    /// `ifOperStatus` (1 = up, 2 = down, …).
    pub oper_status: Option<i64>,
}

impl Port {
    /// Creates an empty port with the given `ifIndex`.
    #[must_use]
    pub fn new(index: u64) -> Self {
        Self {
            index,
            ..Self::default()
        }
    }
}

/// A full network-device inventory result.
///
/// `Default` yields an empty device that MIB modules progressively fill in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkDevice {
    /// Base device identity and attributes.
    pub info: DeviceInfo,
    /// Network interfaces, ordered by `ifIndex`.
    pub ports: Vec<Port>,
}
