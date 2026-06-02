// SPDX-License-Identifier: GPL-2.0-only

//! Tiesse vendor MIB support (networking).
//!
//! Applies to Tiesse devices (`1.3.6.1.4.1.4799`) and fills the `NETWORKING`
//! type, manufacturer, firmware, serial and model. The model comes from the
//! Tiesse private physical description (falling back to `entPhysicalDescr`),
//! keeping the two words after the leading "Tiesse" vendor token. Ported from
//! the upstream `GLPI::Agent::SNMP::MibSupport::Tiesse`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Tiesse enterprise OID.
const TIESSE: &str = "1.3.6.1.4.1.4799";
/// `privatePhysicalDescr` (`tiesse.3.2.6023.0`).
const PRIVATE_PHYSICAL_DESCR: [u64; 11] = [1, 3, 6, 1, 4, 1, 4799, 3, 2, 6023, 0];
/// `privateFirmware` (`tiesse.200.1.0`).
const PRIVATE_FIRMWARE: [u64; 10] = [1, 3, 6, 1, 4, 1, 4799, 200, 1, 0];
/// `privateSerialNumber` (`tiesse.200.2.0`).
const PRIVATE_SERIAL_NUMBER: [u64; 10] = [1, 3, 6, 1, 4, 1, 4799, 200, 2, 0];
/// `entPhysicalDescr.0` (ENTITY-MIB fallback).
const ENT_PHYSICAL_DESCR: [u64; 13] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 2, 0];

/// Vendor MIB module for Tiesse devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct TiesseMib;

#[async_trait]
impl MibSupport for TiesseMib {
    fn name(&self) -> &'static str {
        "tiesse"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), TIESSE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Tiesse".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &PRIVATE_FIRMWARE).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &PRIVATE_SERIAL_NUMBER).await?;
        }
        if device.info.model.is_none() {
            let raw = match get_string(session, &PRIVATE_PHYSICAL_DESCR).await? {
                Some(descr) => Some(descr),
                None => get_string(session, &ENT_PHYSICAL_DESCR).await?,
            };
            device.info.model = raw.map(|descr| strip_tiesse_vendor(&descr));
        }
        Ok(())
    }
}

/// For a "Tiesse <model words…>" description, keeps the two words following the
/// vendor token (the upstream `^(?:\S+) (\S+\s\S+)` capture); other strings are
/// returned unchanged.
fn strip_tiesse_vendor(descr: &str) -> String {
    if descr.to_lowercase().starts_with("tiesse") {
        let words: Vec<&str> = descr.split_whitespace().collect();
        if let (Some(a), Some(b)) = (words.get(1), words.get(2)) {
            return format!("{a} {b}");
        }
    }
    descr.to_owned()
}

#[cfg(test)]
mod tests {
    use super::TiesseMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_tiesse() {
        assert!(TiesseMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.4799.2.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!TiesseMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.4798".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_extracts_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.4799.3.2.6023.0 = STRING: \"Tiesse Imola 5400\"\n\
             .1.3.6.1.4.1.4799.200.1.0 = STRING: \"3.7.1\"\n\
             .1.3.6.1.4.1.4799.200.2.0 = STRING: \"TS123456\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        TiesseMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Tiesse"));
        assert_eq!(device.info.firmware.as_deref(), Some("3.7.1"));
        assert_eq!(device.info.serial.as_deref(), Some("TS123456"));
        assert_eq!(device.info.model.as_deref(), Some("Imola 5400"));
    }
}
