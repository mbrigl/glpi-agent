// SPDX-License-Identifier: GPL-2.0-only

//! Cisco vendor MIB support.
//!
//! Applies to devices under the Cisco enterprise (`1.3.6.1.4.1.9`). Fills the
//! chassis serial number from `OLD-CISCO-CHASSIS-MIB` (`chassisId`,
//! `1.3.6.1.4.1.9.3.6.3.0`) when the standard `ENTITY-MIB` did not provide one
//! — many older Catalyst/IOS devices expose the serial only there.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Cisco Systems enterprise OID.
const CISCO_ENTERPRISE: &str = "1.3.6.1.4.1.9";
/// `OLD-CISCO-CHASSIS-MIB::chassisId.0` — the chassis serial number.
const OLD_CISCO_CHASSIS_SERIAL: [u64; 11] = [1, 3, 6, 1, 4, 1, 9, 3, 6, 3, 0];

/// Vendor MIB module for Cisco devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct CiscoMib;

#[async_trait]
impl MibSupport for CiscoMib {
    fn name(&self) -> &'static str {
        "cisco"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), CISCO_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &OLD_CISCO_CHASSIS_SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CiscoMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_cisco() {
        let cisco = DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        };
        let juniper = DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2636.1.1".to_owned()),
            ..DeviceInfo::default()
        };
        assert!(CiscoMib.applies_to(&cisco));
        assert!(!CiscoMib.applies_to(&juniper));
    }

    #[tokio::test]
    async fn fills_serial_from_old_chassis_mib_when_absent() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.9.3.6.3.0 = STRING: \"CAT0934X5YZ\"\n").unwrap();
        let mut device = NetworkDevice::default();
        CiscoMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("CAT0934X5YZ"));
    }

    #[tokio::test]
    async fn does_not_overwrite_an_existing_serial() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.9.3.6.3.0 = STRING: \"OLD123\"\n").unwrap();
        let mut device = NetworkDevice::default();
        device.info.serial = Some("FROM-ENTITY".to_owned());
        CiscoMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("FROM-ENTITY"));
    }
}
