// SPDX-License-Identifier: GPL-2.0-only

//! Sophos vendor MIB support.
//!
//! Applies to Sophos XG firewalls under `SFOS-FIREWALL-MIB` (`sfosXGMIB`,
//! `1.3.6.1.4.1.2604.5`). Fills the model, firmware version and serial number
//! (`sfosDeviceType`, `sfosDeviceFWVersion`, `sfosDeviceAppKey`). Ported from
//! the upstream `GLPI::Agent::SNMP::MibSupport::Sophos`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `SFOS-FIREWALL-MIB::sfosXGMIB` OID.
const SOPHOS_XG_MIB: &str = "1.3.6.1.4.1.2604.5";
/// `sfosDeviceType.0` — the appliance model.
const SFOS_DEVICE_TYPE: [u64; 12] = [1, 3, 6, 1, 4, 1, 2604, 5, 1, 1, 2, 0];
/// `sfosDeviceFWVersion.0` — the firmware version.
const SFOS_DEVICE_FW_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 2604, 5, 1, 1, 3, 0];
/// `sfosDeviceAppKey.0` — used as the serial number.
const SFOS_DEVICE_APP_KEY: [u64; 12] = [1, 3, 6, 1, 4, 1, 2604, 5, 1, 1, 4, 0];

/// Vendor MIB module for Sophos XG firewalls.
#[derive(Debug, Default, Clone, Copy)]
pub struct SophosMib;

#[async_trait]
impl MibSupport for SophosMib {
    fn name(&self) -> &'static str {
        "sophos"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), SOPHOS_XG_MIB)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &SFOS_DEVICE_TYPE).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &SFOS_DEVICE_FW_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SFOS_DEVICE_APP_KEY).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SophosMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_sophos_xg() {
        assert!(SophosMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2604.5".to_owned()),
            ..DeviceInfo::default()
        }));
        // Sophos enterprise but not the XG firewall subtree.
        assert!(!SophosMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2604.1".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_firmware_and_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.2604.5.1.1.2.0 = STRING: \"XG210\"\n\
             .1.3.6.1.4.1.2604.5.1.1.3.0 = STRING: \"SFOS 19.5.1 GA\"\n\
             .1.3.6.1.4.1.2604.5.1.1.4.0 = STRING: \"C12345ABCDEF67\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        SophosMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("XG210"));
        assert_eq!(device.info.firmware.as_deref(), Some("SFOS 19.5.1 GA"));
        assert_eq!(device.info.serial.as_deref(), Some("C12345ABCDEF67"));
    }
}
