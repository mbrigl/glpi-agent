// SPDX-License-Identifier: GPL-2.0-only

//! SonicWall vendor MIB support.
//!
//! Applies to SonicWall appliances (`SONICWALL-FIREWALL-IP-STATISTICS-MIB`,
//! `1.3.6.1.4.1.8741`). Fills the model, serial number and firmware (ROM)
//! version from `snwlSys`. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::SonicWall`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// SonicWall enterprise OID.
const SONICWALL_ENTERPRISE: &str = "1.3.6.1.4.1.8741";
/// `snwlSysModel.0` (`sonicwall.2.1.1.1.0`).
const SNWL_SYS_MODEL: [u64; 12] = [1, 3, 6, 1, 4, 1, 8741, 2, 1, 1, 1, 0];
/// `snwlSysSerialNumber.0` (`sonicwall.2.1.1.2.0`).
const SNWL_SYS_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 8741, 2, 1, 1, 2, 0];
/// `snwlSysROMVersion.0` (`sonicwall.2.1.1.4.0`).
const SNWL_SYS_ROM_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 8741, 2, 1, 1, 4, 0];

/// Vendor MIB module for SonicWall appliances.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonicWallMib;

#[async_trait]
impl MibSupport for SonicWallMib {
    fn name(&self) -> &'static str {
        "sonicwall"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), SONICWALL_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &SNWL_SYS_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SNWL_SYS_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &SNWL_SYS_ROM_VERSION).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SonicWallMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_sonicwall() {
        assert!(SonicWallMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.8741.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!SonicWallMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_serial_and_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.8741.2.1.1.1.0 = STRING: \"TZ400\"\n\
             .1.3.6.1.4.1.8741.2.1.1.2.0 = STRING: \"18B16900ABCD\"\n\
             .1.3.6.1.4.1.8741.2.1.1.4.0 = STRING: \"SonicROM 7.0.1\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        SonicWallMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("TZ400"));
        assert_eq!(device.info.serial.as_deref(), Some("18B16900ABCD"));
        assert_eq!(device.info.firmware.as_deref(), Some("SonicROM 7.0.1"));
    }
}
