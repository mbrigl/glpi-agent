// SPDX-License-Identifier: GPL-2.0-only

//! Fortinet vendor MIB support.
//!
//! Applies to devices under the Fortinet enterprise (`1.3.6.1.4.1.12356`).
//! Fills the chassis serial number from `FORTINET-CORE-MIB::fnSysSerial`
//! (`1.3.6.1.4.1.12356.100.1.1.1.0`) when `ENTITY-MIB` did not provide one.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Fortinet enterprise OID.
const FORTINET_ENTERPRISE: &str = "1.3.6.1.4.1.12356";
/// `FORTINET-CORE-MIB::fnSysSerial.0` — the appliance serial number.
const FN_SYS_SERIAL: [u64; 12] = [1, 3, 6, 1, 4, 1, 12356, 100, 1, 1, 1, 0];

/// Vendor MIB module for Fortinet devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct FortinetMib;

#[async_trait]
impl MibSupport for FortinetMib {
    fn name(&self) -> &'static str {
        "fortinet"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), FORTINET_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &FN_SYS_SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FortinetMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_fortinet() {
        assert!(FortinetMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.12356.101.1.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!FortinetMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_serial_from_fn_sys_serial() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.12356.100.1.1.1.0 = STRING: \"FGT60D1234567890\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        FortinetMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("FGT60D1234567890"));
    }
}
