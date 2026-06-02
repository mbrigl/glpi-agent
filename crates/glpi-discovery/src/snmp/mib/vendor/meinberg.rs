// SPDX-License-Identifier: GPL-2.0-only

//! Meinberg time-server vendor MIB support (networking).
//!
//! Applies to Meinberg devices (`MBG-SNMP-ROOT-MIB`, `1.3.6.1.4.1.5597`) and
//! sets the `NETWORKING` type, manufacturer, model (from the system
//! description), serial and firmware (from the LANTIME-NG info group). Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::Meinberg`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `mbgSnmpRoot` — Meinberg enterprise OID.
const MBG_SNMP_ROOT: &str = "1.3.6.1.4.1.5597";
/// `mbgLtNgFirmwareVersion` (`mbgLtNgInfo.2.0`).
const MBG_LT_NG_FIRMWARE_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 5597, 30, 0, 0, 2, 0];
/// `mbgLtNgSerialNumber` (`mbgLtNgInfo.3.0`).
const MBG_LT_NG_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 5597, 30, 0, 0, 3, 0];

/// Vendor MIB module for Meinberg devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct MeinbergMib;

#[async_trait]
impl MibSupport for MeinbergMib {
    fn name(&self) -> &'static str {
        "meinberg"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), MBG_SNMP_ROOT)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Meinberg".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = device
                .info
                .description
                .as_deref()
                .and_then(model_from_description);
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &MBG_LT_NG_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &MBG_LT_NG_FIRMWARE_VERSION).await?;
        }
        Ok(())
    }
}

/// Extracts the model from a `Meinberg <model> V<version>` description.
fn model_from_description(description: &str) -> Option<String> {
    let rest = description.strip_prefix("Meinberg ")?;
    let model = rest.rsplit_once(" V")?.0.trim();
    (!model.is_empty()).then(|| model.to_owned())
}

#[cfg(test)]
mod tests {
    use super::MeinbergMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_meinberg() {
        assert!(MeinbergMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.5597.30".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!MeinbergMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.5598".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_model_from_description() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.5597.30.0.0.2.0 = STRING: \"7.06.012\"\n\
             .1.3.6.1.4.1.5597.30.0.0.3.0 = STRING: \"02A1234567\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("Meinberg LANTIME M300 V7.06".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        MeinbergMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Meinberg"));
        assert_eq!(device.info.model.as_deref(), Some("LANTIME M300"));
        assert_eq!(device.info.serial.as_deref(), Some("02A1234567"));
        assert_eq!(device.info.firmware.as_deref(), Some("7.06.012"));
    }
}
