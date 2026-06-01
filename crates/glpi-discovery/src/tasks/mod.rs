// SPDX-License-Identifier: GPL-2.0-only

//! Discovery tasks: the top-level flows that orchestrate the scanner, the
//! discovery methods and the SNMP stack.
//!
//! Currently available:
//!
//! - [`net_discovery`] — the NetDiscovery task: scan address ranges, detect
//!   live hosts and classify SNMP devices into [`DiscoveredDevice`] records.
//!
//! [`DiscoveredDevice`]: net_discovery::DiscoveredDevice

pub mod net_discovery;

pub use net_discovery::{
    discover_snmp, DiscoveredDevice, NetDiscoveryTask, SnmpDevice, SYS_CONTACT, SYS_LOCATION,
};
