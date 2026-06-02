// SPDX-License-Identifier: GPL-2.0-only

//! Eaton vendor MIB support.
//!
//! Covers two Eaton device families, selected by `sysObjectID`:
//!
//! * Eaton ePDU (`EATON-EPDU-MIB`, `1.3.6.1.4.1.534.6.6.7`) — model, serial and
//!   firmware from the unit table;
//! * Eaton/Powerware UPS (`XUPS-MIB`, `1.3.6.1.4.1.534.1`) — manufacturer,
//!   model, software version and serial from `xupsIdent`.
//!
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::EatonEpdu`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `EATON-EPDU-MIB` root OID.
const EPDU_ENTERPRISE: &str = "1.3.6.1.4.1.534.6.6.7";
/// `XUPS-MIB` root OID.
const XUPS_ENTERPRISE: &str = "1.3.6.1.4.1.534.1";

/// ePDU `unitName`-table model (`epdu.1.2.1.3.0`).
const EPDU_MODEL: [u64; 15] = [1, 3, 6, 1, 4, 1, 534, 6, 6, 7, 1, 2, 1, 3, 0];
/// ePDU serial (`epdu.1.2.1.4.0`).
const EPDU_SERIAL: [u64; 15] = [1, 3, 6, 1, 4, 1, 534, 6, 6, 7, 1, 2, 1, 4, 0];
/// ePDU firmware (`epdu.1.2.1.5.0`).
const EPDU_FIRMWARE: [u64; 15] = [1, 3, 6, 1, 4, 1, 534, 6, 6, 7, 1, 2, 1, 5, 0];

/// `XUPS-MIB::xupsIdentManufacturer.0`.
const XUPS_MANUFACTURER: [u64; 11] = [1, 3, 6, 1, 4, 1, 534, 1, 1, 1, 0];
/// `XUPS-MIB::xupsIdentModel.0`.
const XUPS_MODEL: [u64; 11] = [1, 3, 6, 1, 4, 1, 534, 1, 1, 2, 0];
/// `XUPS-MIB::xupsIdentSoftwareVersion.0`.
const XUPS_SOFTWARE_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 534, 1, 1, 3, 0];
/// `XUPS-MIB::xupsIdentSerialNumber.0`.
const XUPS_SERIAL_NUMBER: [u64; 11] = [1, 3, 6, 1, 4, 1, 534, 1, 1, 6, 0];

/// Vendor MIB module for Eaton ePDU and UPS devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct EatonMib;

#[async_trait]
impl MibSupport for EatonMib {
    fn name(&self) -> &'static str {
        "eaton"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, EPDU_ENTERPRISE) || sysobjectid_matches(oid, XUPS_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let is_xups = sysobjectid_matches(device.info.sys_object_id.as_deref(), XUPS_ENTERPRISE);
        let (model_oid, serial_oid, firmware_oid): (&[u64], &[u64], &[u64]) = if is_xups {
            if device.info.manufacturer.is_none() {
                device.info.manufacturer = get_string(session, &XUPS_MANUFACTURER).await?;
            }
            (&XUPS_MODEL, &XUPS_SERIAL_NUMBER, &XUPS_SOFTWARE_VERSION)
        } else {
            (&EPDU_MODEL, &EPDU_SERIAL, &EPDU_FIRMWARE)
        };

        if device.info.model.is_none() {
            device.info.model = get_string(session, model_oid).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, serial_oid).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, firmware_oid).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::EatonMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    fn device_with_oid(oid: &str) -> NetworkDevice {
        NetworkDevice {
            info: DeviceInfo {
                sys_object_id: Some(oid.to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        }
    }

    #[test]
    fn applies_to_epdu_and_xups() {
        assert!(EatonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.534.6.6.7.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(EatonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.534.1.2".to_owned()),
            ..DeviceInfo::default()
        }));
        // Eaton enterprise but neither ePDU nor XUPS subtree.
        assert!(!EatonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.534.10".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_epdu_fields() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.534.6.6.7.1.2.1.3.0 = STRING: \"EMAB03\"\n\
             .1.3.6.1.4.1.534.6.6.7.1.2.1.4.0 = STRING: \"WA00A12345\"\n\
             .1.3.6.1.4.1.534.6.6.7.1.2.1.5.0 = STRING: \"02.00.0051\"\n",
        )
        .unwrap();
        let mut device = device_with_oid("1.3.6.1.4.1.534.6.6.7.1");
        EatonMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("EMAB03"));
        assert_eq!(device.info.serial.as_deref(), Some("WA00A12345"));
        assert_eq!(device.info.firmware.as_deref(), Some("02.00.0051"));
    }

    #[tokio::test]
    async fn fills_xups_fields() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.534.1.1.1.0 = STRING: \"EATON\"\n\
             .1.3.6.1.4.1.534.1.1.2.0 = STRING: \"9PX6000\"\n\
             .1.3.6.1.4.1.534.1.1.3.0 = STRING: \"1.07\"\n\
             .1.3.6.1.4.1.534.1.1.6.0 = STRING: \"G123A45678\"\n",
        )
        .unwrap();
        let mut device = device_with_oid("1.3.6.1.4.1.534.1.2");
        EatonMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("EATON"));
        assert_eq!(device.info.model.as_deref(), Some("9PX6000"));
        assert_eq!(device.info.firmware.as_deref(), Some("1.07"));
        assert_eq!(device.info.serial.as_deref(), Some("G123A45678"));
    }
}
