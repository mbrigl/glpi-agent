// SPDX-License-Identifier: GPL-2.0-only

//! Dell vendor MIB support (networking).
//!
//! Applies to Dell PowerConnect switches (`1.3.6.1.4.1.674.10895.3000`) and
//! Dell OS10 products (`1.3.6.1.4.1.674.11000.5000.100.2`). Sets the
//! `NETWORKING` type and fills manufacturer, model, firmware and serial from
//! the PowerConnect `productIdentification` group, falling back to the OS10
//! chassis PPID for the serial. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Dell`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Dell PowerConnect vendor MIB OID.
const DELL_POWERCONNECT: &str = "1.3.6.1.4.1.674.10895.3000";
/// Dell OS10 products OID.
const DELL_OS10_PRODUCTS: &str = "1.3.6.1.4.1.674.11000.5000.100.2";

/// `productIdentificationDisplayName.0`.
const PRODUCT_DISPLAY_NAME: [u64; 14] = [1, 3, 6, 1, 4, 1, 674, 10895, 3000, 1, 2, 100, 1, 0];
/// `productIdentificationVendor.0`.
const PRODUCT_VENDOR: [u64; 14] = [1, 3, 6, 1, 4, 1, 674, 10895, 3000, 1, 2, 100, 3, 0];
/// `productIdentificationVersion.0`.
const PRODUCT_VERSION: [u64; 14] = [1, 3, 6, 1, 4, 1, 674, 10895, 3000, 1, 2, 100, 4, 0];
/// `productIdentificationSerialNumber` (`…100.8.1.2.1`).
const PRODUCT_SERIAL_NUMBER: [u64; 16] =
    [1, 3, 6, 1, 4, 1, 674, 10895, 3000, 1, 2, 100, 8, 1, 2, 1];
/// `os10ChassisPPID` (`…100.4.1.1.3.1.5.1`).
const OS10_CHASSIS_PPID: [u64; 17] = [1, 3, 6, 1, 4, 1, 674, 11000, 5000, 100, 4, 1, 1, 3, 1, 5, 1];

/// Vendor MIB module for Dell networking devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct DellMib;

#[async_trait]
impl MibSupport for DellMib {
    fn name(&self) -> &'static str {
        "dell"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, DELL_POWERCONNECT) || sysobjectid_matches(oid, DELL_OS10_PRODUCTS)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(
                get_string(session, &PRODUCT_VENDOR)
                    .await?
                    .unwrap_or_else(|| "Dell".to_owned()),
            );
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PRODUCT_DISPLAY_NAME).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &PRODUCT_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = match get_string(session, &PRODUCT_SERIAL_NUMBER).await? {
                Some(serial) => Some(serial),
                None => get_string(session, &OS10_CHASSIS_PPID).await?,
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DellMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_powerconnect_and_os10() {
        assert!(DellMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.674.10895.3000.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(DellMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.674.11000.5000.100.2.1".to_owned()),
            ..DeviceInfo::default()
        }));
        // Dell enterprise but neither supported subtree (e.g. iDRAC).
        assert!(!DellMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.674.10892.5".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_powerconnect_identity() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.674.10895.3000.1.2.100.1.0 = STRING: \"PowerConnect 5548\"\n\
             .1.3.6.1.4.1.674.10895.3000.1.2.100.3.0 = STRING: \"Dell\"\n\
             .1.3.6.1.4.1.674.10895.3000.1.2.100.4.0 = STRING: \"3.0.1.2\"\n\
             .1.3.6.1.4.1.674.10895.3000.1.2.100.8.1.2.1 = STRING: \"CN0ABCD123\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        DellMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Dell"));
        assert_eq!(device.info.model.as_deref(), Some("PowerConnect 5548"));
        assert_eq!(device.info.firmware.as_deref(), Some("3.0.1.2"));
        assert_eq!(device.info.serial.as_deref(), Some("CN0ABCD123"));
    }

    #[tokio::test]
    async fn falls_back_to_os10_ppid_for_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.674.11000.5000.100.4.1.1.3.1.5.1 = STRING: \"CN-0PPID-12345\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        DellMib.run(&mut session, &mut device).await.unwrap();
        // No productIdentificationVendor present, so the default applies.
        assert_eq!(device.info.manufacturer.as_deref(), Some("Dell"));
        assert_eq!(device.info.serial.as_deref(), Some("CN-0PPID-12345"));
    }
}
