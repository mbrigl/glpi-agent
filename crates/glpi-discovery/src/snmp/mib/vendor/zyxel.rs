// SPDX-License-Identifier: GPL-2.0-only

//! Zyxel vendor MIB support.
//!
//! Applies to Zyxel enterprise-solution devices (`1.3.6.1.4.1.890.1.15`). Sets
//! the manufacturer and fills the model, serial and firmware from `esSysInfo`.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Zyxel`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Zyxel `enterpriseSolution` OID (`zyxel.1.15`).
const ZYXEL_ENTERPRISE_SOLUTION: &str = "1.3.6.1.4.1.890.1.15";
/// `sysSwVersionString.0`.
const SYS_SW_VERSION_STRING: [u64; 13] = [1, 3, 6, 1, 4, 1, 890, 1, 15, 3, 1, 6, 0];
/// `sysProductModel.0`.
const SYS_PRODUCT_MODEL: [u64; 13] = [1, 3, 6, 1, 4, 1, 890, 1, 15, 3, 1, 11, 0];
/// `sysProductSerialNumber.0`.
const SYS_PRODUCT_SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 890, 1, 15, 3, 1, 12, 0];

/// Vendor MIB module for Zyxel devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZyxelMib;

#[async_trait]
impl MibSupport for ZyxelMib {
    fn name(&self) -> &'static str {
        "zyxel"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), ZYXEL_ENTERPRISE_SOLUTION)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Zyxel".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &SYS_PRODUCT_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SYS_PRODUCT_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &SYS_SW_VERSION_STRING).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ZyxelMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_zyxel_enterprise_solution() {
        assert!(ZyxelMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.890.1.15.3".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!ZyxelMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_serial_and_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.890.1.15.3.1.6.0 = STRING: \"V4.70(ABXS.0)\"\n\
             .1.3.6.1.4.1.890.1.15.3.1.11.0 = STRING: \"GS1920-24\"\n\
             .1.3.6.1.4.1.890.1.15.3.1.12.0 = STRING: \"S123L45678900\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        ZyxelMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Zyxel"));
        assert_eq!(device.info.model.as_deref(), Some("GS1920-24"));
        assert_eq!(device.info.serial.as_deref(), Some("S123L45678900"));
        assert_eq!(device.info.firmware.as_deref(), Some("V4.70(ABXS.0)"));
    }
}
