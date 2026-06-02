// SPDX-License-Identifier: GPL-2.0-only

//! Radware (Alteon) vendor MIB support (load balancer).
//!
//! Applies to Alteon/Radware devices (`1.3.6.1.4.1.1872`). Fills the
//! manufacturer, model (`Alteon <platform>`), serial and PLD firmware from the
//! `ALTEON-CHEETAH-SWITCH-MIB` hardware group, and records the mainboard and
//! hardware revisions as firmware entries. The device-level MAC/IP that the
//! upstream module also reports are not modelled here. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Radware`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Alteon (Radware) enterprise OID.
const ALTEON: &str = "1.3.6.1.4.1.1872";
/// `agPlatformIdentifier` (`agSystem.77.0`).
const AG_PLATFORM_IDENTIFIER: [u64; 14] = [1, 3, 6, 1, 4, 1, 1872, 2, 5, 1, 1, 1, 77, 0];
/// `hwMainBoardNumber` (`hardware.6.0`).
const HW_MAIN_BOARD_NUMBER: [u64; 14] = [1, 3, 6, 1, 4, 1, 1872, 2, 5, 1, 3, 1, 6, 0];
/// `hwMainBoardRevision` (`hardware.7.0`).
const HW_MAIN_BOARD_REVISION: [u64; 14] = [1, 3, 6, 1, 4, 1, 1872, 2, 5, 1, 3, 1, 7, 0];
/// `hwSerialNumber` (`hardware.18.0`).
const HW_SERIAL_NUMBER: [u64; 14] = [1, 3, 6, 1, 4, 1, 1872, 2, 5, 1, 3, 1, 18, 0];
/// `hwPLDFirmwareVersion` (`hardware.21.0`).
const HW_PLD_FIRMWARE_VERSION: [u64; 14] = [1, 3, 6, 1, 4, 1, 1872, 2, 5, 1, 3, 1, 21, 0];
/// `hwVersion` (`hardware.30.0`).
const HW_VERSION: [u64; 14] = [1, 3, 6, 1, 4, 1, 1872, 2, 5, 1, 3, 1, 30, 0];

/// Vendor MIB module for Radware (Alteon) devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct RadwareMib;

#[async_trait]
impl MibSupport for RadwareMib {
    fn name(&self) -> &'static str {
        "alteon-radware"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), ALTEON)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Radware".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &HW_PLD_FIRMWARE_VERSION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &AG_PLATFORM_IDENTIFIER)
                .await?
                .map(|platform| format!("Alteon {platform}"));
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &HW_SERIAL_NUMBER).await?;
        }

        let model = device.info.model.clone().unwrap_or_default();
        let mainboard = get_string(session, &HW_MAIN_BOARD_NUMBER).await?;
        let mainboard_rev = get_string(session, &HW_MAIN_BOARD_REVISION).await?;
        if let (Some(number), Some(revision)) = (mainboard, mainboard_rev) {
            device.add_firmware(Firmware {
                name: Some(format!("{model} {number} mainboard")),
                description: Some(format!("{model} {number} mainboard revision")),
                r#type: Some("mainboard".to_owned()),
                version: Some(revision),
                manufacturer: Some("Radware".to_owned()),
            });
        }
        if let Some(version) = get_string(session, &HW_VERSION).await? {
            device.add_firmware(Firmware {
                name: Some(format!("{model} hardware")),
                description: Some(format!("{model} hardware revision")),
                r#type: Some("device".to_owned()),
                version: Some(version),
                manufacturer: Some("Radware".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RadwareMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_alteon() {
        assert!(RadwareMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1872.2.5".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RadwareMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1873".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_hardware_firmwares() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1872.2.5.1.1.1.77.0 = STRING: \"5208\"\n\
             .1.3.6.1.4.1.1872.2.5.1.3.1.6.0 = STRING: \"BoardX\"\n\
             .1.3.6.1.4.1.1872.2.5.1.3.1.7.0 = STRING: \"3\"\n\
             .1.3.6.1.4.1.1872.2.5.1.3.1.18.0 = STRING: \"RW1234567\"\n\
             .1.3.6.1.4.1.1872.2.5.1.3.1.21.0 = STRING: \"2.1\"\n\
             .1.3.6.1.4.1.1872.2.5.1.3.1.30.0 = STRING: \"A\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        RadwareMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Radware"));
        assert_eq!(device.info.model.as_deref(), Some("Alteon 5208"));
        assert_eq!(device.info.serial.as_deref(), Some("RW1234567"));
        assert_eq!(device.info.firmware.as_deref(), Some("2.1"));
        assert_eq!(device.firmwares.len(), 2);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("Alteon 5208 BoardX mainboard")
        );
        assert_eq!(device.firmwares[0].version.as_deref(), Some("3"));
        assert_eq!(device.firmwares[1].version.as_deref(), Some("A"));
    }
}
