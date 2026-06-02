// SPDX-License-Identifier: GPL-2.0-only

//! Infortrend vendor MIB support.
//!
//! Applies to Infortrend storage controllers (`1.3.6.1.4.1.1714.1.1`). Sets the
//! `STORAGE` type and manufacturer and fills the model, serial and firmware
//! (the major and minor version joined with a dot) from `sysInformation`.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Infortrend` (the
//! per-disk `STORAGES` enumeration is not modelled here).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Infortrend `extInterface` OID (`infortrend.1.1`).
const INFORTREND_EXT_INTERFACE: &str = "1.3.6.1.4.1.1714.1.1";
/// `fwMajorVersion.0` (`sysInformation.4.0`).
const FW_MAJOR_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 1714, 1, 1, 1, 1, 4, 0];
/// `fwMinorVersion.0` (`sysInformation.5.0`).
const FW_MINOR_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 1714, 1, 1, 1, 1, 5, 0];
/// `serialNum.0` (`sysInformation.10.0`).
const SERIAL_NUM: [u64; 13] = [1, 3, 6, 1, 4, 1, 1714, 1, 1, 1, 1, 10, 0];
/// `privateLogoModel.0` (`sysInformation.15.0`).
const PRIVATE_LOGO_MODEL: [u64; 13] = [1, 3, 6, 1, 4, 1, 1714, 1, 1, 1, 1, 15, 0];

/// Vendor MIB module for Infortrend storage controllers.
#[derive(Debug, Default, Clone, Copy)]
pub struct InfortrendMib;

#[async_trait]
impl MibSupport for InfortrendMib {
    fn name(&self) -> &'static str {
        "infortrend"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), INFORTREND_EXT_INTERFACE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("STORAGE".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Infortrend Technology, Inc.".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PRIVATE_LOGO_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL_NUM).await?;
        }
        if device.info.firmware.is_none() {
            let major = get_string(session, &FW_MAJOR_VERSION).await?;
            let minor = get_string(session, &FW_MINOR_VERSION).await?;
            device.info.firmware = match (major, minor) {
                (Some(major), Some(minor)) => Some(format!("{major}.{minor}")),
                (Some(version), None) | (None, Some(version)) => Some(version),
                (None, None) => None,
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::InfortrendMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_infortrend() {
        assert!(InfortrendMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1714.1.1.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!InfortrendMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_joins_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1714.1.1.1.1.4.0 = STRING: \"7\"\n\
             .1.3.6.1.4.1.1714.1.1.1.1.5.0 = STRING: \"00\"\n\
             .1.3.6.1.4.1.1714.1.1.1.1.10.0 = STRING: \"8123456\"\n\
             .1.3.6.1.4.1.1714.1.1.1.1.15.0 = STRING: \"EonStor DS 3024\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        InfortrendMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("STORAGE"));
        assert_eq!(
            device.info.manufacturer.as_deref(),
            Some("Infortrend Technology, Inc.")
        );
        assert_eq!(device.info.model.as_deref(), Some("EonStor DS 3024"));
        assert_eq!(device.info.serial.as_deref(), Some("8123456"));
        assert_eq!(device.info.firmware.as_deref(), Some("7.00"));
    }
}
