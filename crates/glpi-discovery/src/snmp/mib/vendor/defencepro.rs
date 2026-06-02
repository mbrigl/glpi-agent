// SPDX-License-Identifier: GPL-2.0-only

//! Radware DefencePro vendor MIB support.
//!
//! Applies to DefencePro appliances (`1.3.6.1.4.1.89`) and fills the firmware,
//! serial and model. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::DefencePro`; the device-level MAC is not
//! modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// DefencePro enterprise OID.
const DEFENCEPRO: &str = "1.3.6.1.4.1.89";
/// `model` (`defencepro.2.14.0`).
const MODEL: [u64; 10] = [1, 3, 6, 1, 4, 1, 89, 2, 14, 0];
/// `rndSerialNumber` (`defencepro.2.12.0`).
const RND_SERIAL_NUMBER: [u64; 10] = [1, 3, 6, 1, 4, 1, 89, 2, 12, 0];
/// `rsWSDUserVersion` (`defencepro.35.1.34`).
const RS_WSD_USER_VERSION: [u64; 10] = [1, 3, 6, 1, 4, 1, 89, 35, 1, 34];

/// Vendor MIB module for Radware DefencePro appliances.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefenceProMib;

#[async_trait]
impl MibSupport for DefenceProMib {
    fn name(&self) -> &'static str {
        "DefencePro"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), DEFENCEPRO)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &RS_WSD_USER_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &RND_SERIAL_NUMBER).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &MODEL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DefenceProMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_defencepro() {
        assert!(DefenceProMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.89.2".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!DefenceProMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.8".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_firmware_serial_and_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.89.2.14.0 = STRING: \"DefensePro 6420\"\n\
             .1.3.6.1.4.1.89.2.12.0 = STRING: \"Q2-0123456\"\n\
             .1.3.6.1.4.1.89.35.1.34 = STRING: \"8.13.01\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        DefenceProMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.firmware.as_deref(), Some("8.13.01"));
        assert_eq!(device.info.serial.as_deref(), Some("Q2-0123456"));
        assert_eq!(device.info.model.as_deref(), Some("DefensePro 6420"));
    }
}
