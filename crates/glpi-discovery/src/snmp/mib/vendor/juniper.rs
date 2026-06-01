// SPDX-License-Identifier: GPL-2.0-only

//! Juniper vendor MIB support.
//!
//! Applies to devices under the Juniper enterprise (`1.3.6.1.4.1.2636`). Fills
//! the chassis serial number from `JUNIPER-MIB::jnxBoxSerialNo`
//! (`1.3.6.1.4.1.2636.3.1.3.0`) when `ENTITY-MIB` did not provide one.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Juniper Networks enterprise OID.
const JUNIPER_ENTERPRISE: &str = "1.3.6.1.4.1.2636";
/// `JUNIPER-MIB::jnxBoxSerialNo.0` — the chassis serial number.
const JNX_BOX_SERIAL_NO: [u64; 11] = [1, 3, 6, 1, 4, 1, 2636, 3, 1, 3, 0];

/// Vendor MIB module for Juniper devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct JuniperMib;

#[async_trait]
impl MibSupport for JuniperMib {
    fn name(&self) -> &'static str {
        "juniper"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), JUNIPER_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &JNX_BOX_SERIAL_NO).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JuniperMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_juniper() {
        let juniper = DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2636.1.1.1.2.30".to_owned()),
            ..DeviceInfo::default()
        };
        assert!(JuniperMib.applies_to(&juniper));
        assert!(!JuniperMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_serial_from_jnx_box_serial() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.2636.3.1.3.0 = STRING: \"JN123ABC456\"\n").unwrap();
        let mut device = NetworkDevice::default();
        JuniperMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("JN123ABC456"));
    }
}
