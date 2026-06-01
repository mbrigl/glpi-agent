// SPDX-License-Identifier: GPL-2.0-only

//! The NetInventory result types that MIB modules populate.
//!
//! A [`NetworkDevice`] is built up by running [`MibSupport`] modules against a
//! device: the standard MIBs fill the base [`DeviceInfo`] and the port/component
//! tables, and vendor MIBs refine them. The structure mirrors the GLPI network
//! device schema (INFO / PORTS / COMPONENTS) and grows as more MIBs land.
//!
//! [`MibSupport`]: super::MibSupport

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

/// A full network-device inventory result.
///
/// `Default` yields an empty device that MIB modules progressively fill in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkDevice {
    /// Base device identity and attributes.
    pub info: DeviceInfo,
}
