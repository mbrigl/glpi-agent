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

pub mod aerohive;
pub mod avaya;
pub mod avocent;
pub mod bachmann;
pub mod brother;
pub mod canon;
pub mod cisco;
pub mod dell;
pub mod digipower;
pub mod dlink;
pub mod eaton;
pub mod emc;
pub mod epson;
pub mod fortinet;
pub mod foxgate;
pub mod hikvision;
pub mod hitachi_vantara;
pub mod hp_citizen;
pub mod hp_printer;
pub mod htek;
pub mod infortrend;
pub mod intelbras;
pub mod juniper;
pub mod konica;
pub mod kyocera;
pub mod lexmark;
pub mod mikrotik;
pub mod multitech;
pub mod netscaler;
pub mod nokia;
pub mod oki;
pub mod pantum;
pub mod qnap;
pub mod quantum;
pub mod radware;
pub mod raritan;
pub mod ricoh;
pub mod rnx;
pub mod ruckus;
pub mod snom;
pub mod sonicwall;
pub mod sophos;
pub mod telco;
pub mod tiesse;
pub mod toshiba;
pub mod watchguard;
pub mod xerox;
pub mod zebra;
pub mod zyxel;

pub use aerohive::AerohiveMib;
pub use avaya::AvayaMib;
pub use avocent::AvocentMib;
pub use bachmann::BachmannMib;
pub use brother::BrotherMib;
pub use canon::CanonMib;
pub use cisco::CiscoMib;
pub use dell::DellMib;
pub use digipower::DigiPowerMib;
pub use dlink::DlinkMib;
pub use eaton::EatonMib;
pub use emc::EmcMib;
pub use epson::EpsonMib;
pub use fortinet::FortinetMib;
pub use foxgate::FoxGateMib;
pub use hikvision::HikvisionMib;
pub use hitachi_vantara::HitachiVantaraMib;
pub use hp_citizen::HpCitizenMib;
pub use hp_printer::HpPrinterMib;
pub use htek::HtekMib;
pub use infortrend::InfortrendMib;
pub use intelbras::IntelbrasMib;
pub use juniper::JuniperMib;
pub use konica::KonicaMib;
pub use kyocera::KyoceraMib;
pub use lexmark::LexmarkMib;
pub use mikrotik::MikrotikMib;
pub use multitech::MultitechMib;
pub use netscaler::NetscalerMib;
pub use nokia::NokiaMib;
pub use oki::OkiMib;
pub use pantum::PantumMib;
pub use qnap::QnapMib;
pub use quantum::QuantumMib;
pub use radware::RadwareMib;
pub use raritan::RaritanMib;
pub use ricoh::RicohMib;
pub use rnx::RnxMib;
pub use ruckus::RuckusMib;
pub use snom::SnomMib;
pub use sonicwall::SonicWallMib;
pub use sophos::SophosMib;
pub use telco::TelcoMib;
pub use tiesse::TiesseMib;
pub use toshiba::ToshibaMib;
pub use watchguard::WatchGuardMib;
pub use xerox::XeroxMib;
pub use zebra::ZebraMib;
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
    registry.register(Arc::new(AerohiveMib));
    registry.register(Arc::new(DlinkMib));
    registry.register(Arc::new(FoxGateMib));
    registry.register(Arc::new(IntelbrasMib));
    registry.register(Arc::new(NokiaMib));
    registry.register(Arc::new(TelcoMib));
    registry.register(Arc::new(TiesseMib));
    registry.register(Arc::new(WatchGuardMib));
    registry.register(Arc::new(EmcMib));
    registry.register(Arc::new(HitachiVantaraMib));
    registry.register(Arc::new(RadwareMib));
    registry.register(Arc::new(BachmannMib));
    registry.register(Arc::new(RnxMib));
    registry.register(Arc::new(DigiPowerMib));
    registry.register(Arc::new(KyoceraMib));
    registry.register(Arc::new(ToshibaMib));
    registry.register(Arc::new(ZebraMib));
    registry.register(Arc::new(HpCitizenMib));
    registry.register(Arc::new(AvayaMib));
    registry.register(Arc::new(HtekMib));
    registry.register(Arc::new(SnomMib));
    registry.register(Arc::new(MultitechMib));
    registry.register(Arc::new(AvocentMib));
}
