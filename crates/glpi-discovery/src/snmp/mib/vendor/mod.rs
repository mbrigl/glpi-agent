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

pub mod brother;
pub mod canon;
pub mod cisco;
pub mod dell;
pub mod eaton;
pub mod epson;
pub mod fortinet;
pub mod hikvision;
pub mod hp_printer;
pub mod infortrend;
pub mod juniper;
pub mod konica;
pub mod lexmark;
pub mod mikrotik;
pub mod netscaler;
pub mod oki;
pub mod pantum;
pub mod qnap;
pub mod quantum;
pub mod raritan;
pub mod ricoh;
pub mod ruckus;
pub mod sonicwall;
pub mod sophos;
pub mod xerox;
pub mod zyxel;

pub use brother::BrotherMib;
pub use canon::CanonMib;
pub use cisco::CiscoMib;
pub use dell::DellMib;
pub use eaton::EatonMib;
pub use epson::EpsonMib;
pub use fortinet::FortinetMib;
pub use hikvision::HikvisionMib;
pub use hp_printer::HpPrinterMib;
pub use infortrend::InfortrendMib;
pub use juniper::JuniperMib;
pub use konica::KonicaMib;
pub use lexmark::LexmarkMib;
pub use mikrotik::MikrotikMib;
pub use netscaler::NetscalerMib;
pub use oki::OkiMib;
pub use pantum::PantumMib;
pub use qnap::QnapMib;
pub use quantum::QuantumMib;
pub use raritan::RaritanMib;
pub use ricoh::RicohMib;
pub use ruckus::RuckusMib;
pub use sonicwall::SonicWallMib;
pub use sophos::SophosMib;
pub use xerox::XeroxMib;
pub use zyxel::ZyxelMib;

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
    registry.register(Arc::new(ZyxelMib));
    registry.register(Arc::new(RaritanMib));
    registry.register(Arc::new(QuantumMib));
    registry.register(Arc::new(InfortrendMib));
    registry.register(Arc::new(OkiMib));
    registry.register(Arc::new(EpsonMib));
    registry.register(Arc::new(CanonMib));
    registry.register(Arc::new(PantumMib));
    registry.register(Arc::new(XeroxMib));
    registry.register(Arc::new(HpPrinterMib));
    registry.register(Arc::new(BrotherMib));
    registry.register(Arc::new(RicohMib));
    registry.register(Arc::new(KonicaMib));
    registry.register(Arc::new(LexmarkMib));
}
