// SPDX-License-Identifier: GPL-2.0-only

//! RNX PDU vendor MIB support.
//!
//! Applies to RNX power-distribution units (`RNX-UPDU-MIB2-MIB`,
//! `1.3.6.1.4.1.55108`). Fills the manufacturer, serial, firmware and model
//! (parsed from the system description). The device type is reported as
//! `NETWORKING` and the per-outlet PDU plug inventory of the upstream module is
//! not modelled (both need GLPI 12 PDU support). Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::RNX`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// RNX enterprise OID.
const RNX: &str = "1.3.6.1.4.1.55108";
/// `upduMib2PDUSerialNumber` (`upduMib2.1.2.1.5.1`).
const UPDU_PDU_SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 55108, 2, 1, 2, 1, 5, 1];
/// `upduMib2ICMFirmware` (`upduMib2.6.2.1.9.1`).
const UPDU_ICM_FIRMWARE: [u64; 13] = [1, 3, 6, 1, 4, 1, 55108, 2, 6, 2, 1, 9, 1];

/// Vendor MIB module for RNX PDUs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RnxMib;

#[async_trait]
impl MibSupport for RnxMib {
    fn name(&self) -> &'static str {
        "rnx-pdu"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), RNX)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("RNX".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &UPDU_PDU_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &UPDU_ICM_FIRMWARE).await?;
        }
        if device.info.model.is_none() {
            device.info.model = device
                .info
                .description
                .as_deref()
                .and_then(model_from_description);
        }
        Ok(())
    }
}

/// Extracts the model from an `RNX <model> (…)` system description.
fn model_from_description(description: &str) -> Option<String> {
    let rest = description.strip_prefix("RNX ")?;
    let model = rest[..rest.rfind(" (")?].trim();
    (!model.is_empty()).then(|| model.to_owned())
}

#[cfg(test)]
mod tests {
    use super::RnxMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_rnx() {
        assert!(RnxMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.55108.2".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RnxMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.55109".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_parses_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.55108.2.1.2.1.5.1 = STRING: \"RNX240100042\"\n\
             .1.3.6.1.4.1.55108.2.6.2.1.9.1 = STRING: \"2.7.0\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("RNX UPDU-3PH-32A (Rev 1.0)".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        RnxMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("RNX"));
        assert_eq!(device.info.serial.as_deref(), Some("RNX240100042"));
        assert_eq!(device.info.firmware.as_deref(), Some("2.7.0"));
        assert_eq!(device.info.model.as_deref(), Some("UPDU-3PH-32A"));
    }
}
