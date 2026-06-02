// SPDX-License-Identifier: GPL-2.0-only

//! Vendor-specific MIB modules, selected by `sysObjectID`.
//!
//! Each module implements [`MibSupport`](super::MibSupport) and overrides
//! `applies_to` to match its vendor's enterprise OID, so it runs only for the
//! relevant devices. [`register_all`] adds every implemented vendor module to a
//! registry (used by [`MibRegistry::with_defaults`](super::MibRegistry::with_defaults)).
//!
//! Ported from the upstream Perl `GLPI::Agent::SNMP::MibSupport::*` modules
//! (exact OIDs) and exercised with representative `snmpwalk` fixtures; the set
//! grows in batches.

use std::sync::Arc;

use super::MibRegistry;

pub mod cisco;
pub mod dell;
pub mod eaton;
pub mod fortinet;
pub mod hikvision;
pub mod juniper;
pub mod mikrotik;
pub mod netscaler;
pub mod qnap;
pub mod ruckus;
pub mod sonicwall;
pub mod sophos;

pub use cisco::CiscoMib;
pub use dell::DellMib;
pub use eaton::EatonMib;
pub use fortinet::FortinetMib;
pub use hikvision::HikvisionMib;
pub use juniper::JuniperMib;
pub use mikrotik::MikrotikMib;
pub use netscaler::NetscalerMib;
pub use qnap::QnapMib;
pub use ruckus::RuckusMib;
pub use sonicwall::SonicWallMib;
pub use sophos::SophosMib;

/// Registers all implemented vendor MIB modules into `registry`.
pub fn register_all(registry: &mut MibRegistry) {
    registry.register(Arc::new(CiscoMib));
    registry.register(Arc::new(JuniperMib));
    registry.register(Arc::new(FortinetMib));
    registry.register(Arc::new(MikrotikMib));
    registry.register(Arc::new(QnapMib));
    registry.register(Arc::new(SophosMib));
    registry.register(Arc::new(HikvisionMib));
    registry.register(Arc::new(EatonMib));
    registry.register(Arc::new(DellMib));
    registry.register(Arc::new(NetscalerMib));
    registry.register(Arc::new(SonicWallMib));
    registry.register(Arc::new(RuckusMib));
}
