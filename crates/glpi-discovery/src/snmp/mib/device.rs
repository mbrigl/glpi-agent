// SPDX-License-Identifier: GPL-2.0-only

//! The NetInventory result types that MIB modules populate.
//!
//! A [`NetworkDevice`] is built up by running [`MibSupport`] modules against a
//! device: the standard MIBs fill the base [`DeviceInfo`] and the port/component
//! tables, and vendor MIBs refine them. The structure mirrors the GLPI network
//! device schema (INFO / PORTS / COMPONENTS) and grows as more MIBs land.
//!
//! [`MibSupport`]: super::MibSupport

use std::net::IpAddr;

use glpi_core::types::network::MacAddress;
use serde::Serialize;

/// Base identity and attributes of a discovered network device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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
    /// Chassis serial number (typically from `ENTITY-MIB` or a vendor MIB).
    pub serial: Option<String>,
    /// Firmware / software revision.
    pub firmware: Option<String>,
}

/// A physical component of a device (a row of `ENTITY-MIB`'s
/// `entPhysicalTable`): chassis, module, power supply, fan, CPU, …
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Component {
    /// `entPhysicalIndex`.
    pub index: u64,
    /// `entPhysicalDescr`.
    pub description: Option<String>,
    /// `entPhysicalName`.
    pub name: Option<String>,
    /// `entPhysicalClass` (3 = chassis, 9 = module, 12 = cpu, …).
    pub class: Option<i64>,
    /// `entPhysicalSerialNum`.
    pub serial: Option<String>,
    /// `entPhysicalModelName`.
    pub model: Option<String>,
    /// `entPhysicalMfgName`.
    pub manufacturer: Option<String>,
    /// `entPhysicalFirmwareRev`.
    pub firmware: Option<String>,
    /// `entPhysicalHardwareRev`.
    pub hardware_rev: Option<String>,
    /// `entPhysicalSoftwareRev`.
    pub software_rev: Option<String>,
}

impl Component {
    /// Creates an empty component with the given `entPhysicalIndex`.
    #[must_use]
    pub fn new(index: u64) -> Self {
        Self {
            index,
            ..Self::default()
        }
    }

    /// `entPhysicalClass` value for a chassis.
    pub const CLASS_CHASSIS: i64 = 3;
}

/// A network interface (a row of `ifTable` / `ifXTable`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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
    /// IP addresses assigned to this interface (`IP-MIB`). Sorted, de-duplicated.
    pub ips: Vec<IpAddr>,
    /// MAC addresses learned on this port via the bridge forwarding database
    /// (`BRIDGE-MIB`), used to derive connections. Sorted and de-duplicated.
    pub connected_macs: Vec<MacAddress>,
    /// Neighbors discovered on this port via LLDP / CDP.
    pub neighbors: Vec<Neighbor>,
}

/// The discovery protocol a [`Neighbor`] was learned from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NeighborProtocol {
    /// IEEE 802.1AB Link Layer Discovery Protocol.
    Lldp,
    /// Cisco Discovery Protocol.
    Cdp,
}

/// A neighboring device discovered on a port via LLDP or CDP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Neighbor {
    /// Which protocol reported this neighbor.
    pub protocol: NeighborProtocol,
    /// Remote chassis identifier (often a MAC address in text form).
    pub chassis_id: Option<String>,
    /// Remote chassis MAC, when the chassis id is a MAC address.
    pub mac: Option<MacAddress>,
    /// Remote system name.
    pub sys_name: Option<String>,
    /// Remote system description.
    pub sys_descr: Option<String>,
    /// Remote port identifier.
    pub port_id: Option<String>,
    /// Remote port description.
    pub port_descr: Option<String>,
}

impl Neighbor {
    /// Creates an empty neighbor for `protocol`.
    #[must_use]
    pub fn new(protocol: NeighborProtocol) -> Self {
        Self {
            protocol,
            chassis_id: None,
            mac: None,
            sys_name: None,
            sys_descr: None,
            port_id: None,
            port_descr: None,
        }
    }

    /// `true` if no remote attribute was populated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chassis_id.is_none()
            && self.sys_name.is_none()
            && self.sys_descr.is_none()
            && self.port_id.is_none()
            && self.port_descr.is_none()
    }
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

/// A printer consumable (a row of `Printer-MIB`'s `prtMarkerSuppliesTable`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Supply {
    /// `prtMarkerSuppliesDescription`.
    pub description: Option<String>,
    /// `prtMarkerSuppliesType` (3 = toner, 4 = wasteToner, …).
    pub r#type: Option<i64>,
    /// `prtMarkerSuppliesLevel` (current level, in `unit`s; -2 = unknown).
    pub level: Option<i64>,
    /// `prtMarkerSuppliesMaxCapacity`.
    pub max_capacity: Option<i64>,
    /// `prtMarkerSuppliesSupplyUnit`.
    pub unit: Option<i64>,
}

/// Per-type page counters (GLPI `PAGECOUNTERS`), populated by vendor MIBs that
/// expose richer counters than the standard `prtMarkerLifeCount`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PageCounters {
    /// `TOTAL` — total pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    /// `BLACK` — black-and-white pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black: Option<i64>,
    /// `COLOR` — color pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<i64>,
    /// `RECTOVERSO` — duplex pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rectoverso: Option<i64>,
    /// `SCANNED` — scanned pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scanned: Option<i64>,
    /// `PRINTTOTAL` — total printed pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_total: Option<i64>,
    /// `PRINTBLACK` — printed black pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_black: Option<i64>,
    /// `PRINTCOLOR` — printed color pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_color: Option<i64>,
    /// `COPYTOTAL` — total copied pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_total: Option<i64>,
    /// `COPYBLACK` — copied black pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_black: Option<i64>,
    /// `COPYCOLOR` — copied color pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_color: Option<i64>,
    /// `FAXTOTAL` — total faxed pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fax_total: Option<i64>,
}

impl PageCounters {
    /// `true` if no counter was populated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == PageCounters::default()
    }
}

/// Printer-specific inventory (RFC 3805 `Printer-MIB`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Printer {
    /// Lifetime page count (`prtMarkerLifeCount`).
    pub total_pages: Option<i64>,
    /// Consumables (toner/ink cartridges, …), ordered by table index.
    pub supplies: Vec<Supply>,
    /// Per-type page counters from vendor MIBs.
    #[serde(skip_serializing_if = "PageCounters::is_empty")]
    pub page_counters: PageCounters,
}

/// A firmware / software entry of a device (GLPI `FIRMWARES`), used by vendor
/// MIBs that report integrated software components (antivirus engines, line
/// cards, sub-system versions, …).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Firmware {
    /// Short name of the firmware entry.
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Firmware kind (`system`, `software`, `service`, `mib`, …).
    pub r#type: Option<String>,
    /// Version string.
    pub version: Option<String>,
    /// Manufacturer.
    pub manufacturer: Option<String>,
}

/// A full network-device inventory result.
///
/// `Default` yields an empty device that MIB modules progressively fill in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NetworkDevice {
    /// Base device identity and attributes.
    pub info: DeviceInfo,
    /// Network interfaces, ordered by `ifIndex`.
    pub ports: Vec<Port>,
    /// Physical components, ordered by `entPhysicalIndex`.
    pub components: Vec<Component>,
    /// Printer details, when the device exposes the `Printer-MIB`.
    pub printer: Option<Printer>,
    /// Firmware / software entries reported by vendor MIBs.
    pub firmwares: Vec<Firmware>,
}

impl NetworkDevice {
    /// Returns the printer record, creating an empty one if absent. Used by
    /// vendor MIBs that contribute page counters to a device the standard
    /// `Printer-MIB` may not have flagged.
    pub fn printer_mut(&mut self) -> &mut Printer {
        self.printer.get_or_insert_with(Printer::default)
    }

    /// Appends a firmware entry.
    pub fn add_firmware(&mut self, firmware: Firmware) {
        self.firmwares.push(firmware);
    }
}
