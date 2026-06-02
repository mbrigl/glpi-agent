// SPDX-License-Identifier: GPL-2.0-only

//! Aerohive Networks vendor MIB support.
//!
//! Applies to Aerohive devices (`1.3.6.1.4.1.26928`) and fills the `NETWORKING`
//! type, manufacturer, serial, firmware and model from the `AH-SYSTEM-MIB`,
//! recording the platform hardware version as a firmware entry. Ported from the
//! upstream `GLPI::Agent::SNMP::MibSupport::Aerohive`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Aerohive enterprise OID.
const AEROHIVE: &str = "1.3.6.1.4.1.26928";
/// `ahSystemSerial` (`ahSystem.5.0`).
const AH_SYSTEM_SERIAL: [u64; 11] = [1, 3, 6, 1, 4, 1, 26928, 1, 2, 5, 0];
/// `ahDeviceMode` (`ahSystem.6.0`) — the model designation.
const AH_DEVICE_MODE: [u64; 11] = [1, 3, 6, 1, 4, 1, 26928, 1, 2, 6, 0];
/// `ahHwVersion` (`ahSystem.8.0`).
const AH_HW_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 26928, 1, 2, 8, 0];
/// `ahFirmwareVersion` (`ahSystem.12.0`).
const AH_FIRMWARE_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 26928, 1, 2, 12, 0];

/// Vendor MIB module for Aerohive Networks devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct AerohiveMib;

#[async_trait]
impl MibSupport for AerohiveMib {
    fn name(&self) -> &'static str {
        "aerohive"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), AEROHIVE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Aerohive Networks".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &AH_SYSTEM_SERIAL).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &AH_FIRMWARE_VERSION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &AH_DEVICE_MODE).await?;
        }

        if let Some(version) = get_string(session, &AH_HW_VERSION).await? {
            device.add_firmware(Firmware {
                name: Some("Aerohive hardware".to_owned()),
                description: Some("Aerohive platform hardware version".to_owned()),
                r#type: Some("hardware".to_owned()),
                version: Some(version),
                manufacturer: Some("Aerohive Networks".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AerohiveMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_aerohive() {
        assert!(AerohiveMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.26928.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!AerohiveMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2692".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_hardware_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.26928.1.2.5.0 = STRING: \"01234567890123\"\n\
             .1.3.6.1.4.1.26928.1.2.6.0 = STRING: \"AP230\"\n\
             .1.3.6.1.4.1.26928.1.2.8.0 = STRING: \"M01\"\n\
             .1.3.6.1.4.1.26928.1.2.12.0 = STRING: \"HiveOS 6.5r3\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        AerohiveMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(
            device.info.manufacturer.as_deref(),
            Some("Aerohive Networks")
        );
        assert_eq!(device.info.serial.as_deref(), Some("01234567890123"));
        assert_eq!(device.info.firmware.as_deref(), Some("HiveOS 6.5r3"));
        assert_eq!(device.info.model.as_deref(), Some("AP230"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("M01"));
    }
}
