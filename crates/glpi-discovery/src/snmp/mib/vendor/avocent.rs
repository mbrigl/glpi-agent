// SPDX-License-Identifier: GPL-2.0-only

//! Avocent console-server vendor MIB support.
//!
//! Applies to Avocent devices (`ACS8000-MIB`, `1.3.6.1.4.1.10418`) and fills the
//! firmware, model, hostname and serial, recording the bootcode version as a
//! firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Avocent`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Avocent enterprise OID.
const AVOCENT: &str = "1.3.6.1.4.1.10418";
/// `acsHostName` (`acsAppliance.1.0`).
const ACS_HOST_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 10418, 26, 2, 1, 1, 0];
/// `acsProductModel` (`acsAppliance.2.0`).
const ACS_PRODUCT_MODEL: [u64; 12] = [1, 3, 6, 1, 4, 1, 10418, 26, 2, 1, 2, 0];
/// `acsSerialNumber` (`acsAppliance.4.0`).
const ACS_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 10418, 26, 2, 1, 4, 0];
/// `acsBootcodeVersion` (`acsAppliance.6.0`).
const ACS_BOOTCODE_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 10418, 26, 2, 1, 6, 0];
/// `acsFirmwareVersion` (`acsAppliance.7.0`).
const ACS_FIRMWARE_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 10418, 26, 2, 1, 7, 0];

/// Vendor MIB module for Avocent console servers.
#[derive(Debug, Default, Clone, Copy)]
pub struct AvocentMib;

#[async_trait]
impl MibSupport for AvocentMib {
    fn name(&self) -> &'static str {
        "avocent"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), AVOCENT)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &ACS_FIRMWARE_VERSION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &ACS_PRODUCT_MODEL).await?;
        }
        if device.info.name.is_none() {
            device.info.name = get_string(session, &ACS_HOST_NAME).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &ACS_SERIAL_NUMBER).await?;
        }

        if let Some(version) = get_string(session, &ACS_BOOTCODE_VERSION).await? {
            let prefix = device
                .info
                .model
                .as_deref()
                .map_or(String::new(), |m| format!("{m} "));
            device.add_firmware(Firmware {
                name: Some(format!("{prefix}bootcode")),
                description: Some("bootcode firmware version".to_owned()),
                r#type: Some("device".to_owned()),
                version: Some(version),
                manufacturer: Some("Avocent".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AvocentMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_avocent() {
        assert!(AvocentMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.10418.26".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!AvocentMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.10419".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_bootcode_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.10418.26.2.1.1.0 = STRING: \"acs-console-1\"\n\
             .1.3.6.1.4.1.10418.26.2.1.2.0 = STRING: \"ACS8048\"\n\
             .1.3.6.1.4.1.10418.26.2.1.4.0 = STRING: \"AVO123456\"\n\
             .1.3.6.1.4.1.10418.26.2.1.6.0 = STRING: \"1.0.0\"\n\
             .1.3.6.1.4.1.10418.26.2.1.7.0 = STRING: \"3.4.0\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        AvocentMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.firmware.as_deref(), Some("3.4.0"));
        assert_eq!(device.info.model.as_deref(), Some("ACS8048"));
        assert_eq!(device.info.name.as_deref(), Some("acs-console-1"));
        assert_eq!(device.info.serial.as_deref(), Some("AVO123456"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("ACS8048 bootcode")
        );
    }
}
