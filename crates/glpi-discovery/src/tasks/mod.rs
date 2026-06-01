// SPDX-License-Identifier: GPL-2.0-only

//! Discovery tasks: the top-level flows that orchestrate the scanner, the
//! discovery methods and the SNMP stack.
//!
//! Currently available:
//!
//! - [`net_discovery`] — the NetDiscovery task: scan address ranges, detect
//!   live hosts and classify SNMP devices into [`DiscoveredDevice`] records,
//! - [`net_inventory`] — the NetInventory task: deep SNMP inventory of a single
//!   device via the MIB registry.
//!
//! [`DiscoveredDevice`]: net_discovery::DiscoveredDevice

pub mod net_discovery;
pub mod net_inventory;

pub use net_discovery::{
    discover_snmp, DiscoveredDevice, NetDiscoveryTask, SnmpDevice, SYS_CONTACT, SYS_LOCATION,
};
pub use net_inventory::NetInventoryTask;
