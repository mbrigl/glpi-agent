// SPDX-License-Identifier: GPL-2.0-only

//! Toshiba TEC printer vendor MIB support.
//!
//! Applies to Toshiba TEC devices (`1.3.6.1.4.1.1129`) and fills the serial and
//! model, recording the product firmware (a leading `B` rewritten to `V`) and
//! the boot-software version as firmware entries. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Toshiba`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Toshiba TEC enterprise OID.
const TOSHIBATEC: &str = "1.3.6.1.4.1.1129";
/// `bcpProductNumber` (`bcpGeneral.1.0`) — the serial.
const BCP_PRODUCT_NUMBER: [u64; 15] = [1, 3, 6, 1, 4, 1, 1129, 1, 2, 1, 1, 1, 1, 1, 0];
/// `bcpProductVersion` (`bcpGeneral.2.0`).
const BCP_PRODUCT_VERSION: [u64; 15] = [1, 3, 6, 1, 4, 1, 1129, 1, 2, 1, 1, 1, 1, 2, 0];
/// `bcpDeviceModel` (`bcpDeviceEntry.1.0`).
const BCP_DEVICE_MODEL: [u64; 15] = [1, 3, 6, 1, 4, 1, 1129, 1, 2, 1, 1, 1, 2, 1, 0];
/// `bcpDeviceBootVersion` (`bcpDeviceEntry.5.0`).
const BCP_DEVICE_BOOT_VERSION: [u64; 15] = [1, 3, 6, 1, 4, 1, 1129, 1, 2, 1, 1, 1, 2, 5, 0];

/// Vendor MIB module for Toshiba TEC printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct ToshibaMib;

#[async_trait]
impl MibSupport for ToshibaMib {
    fn name(&self) -> &'static str {
        "toshiba"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), TOSHIBATEC)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &BCP_PRODUCT_NUMBER).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &BCP_DEVICE_MODEL).await?;
        }

        if let Some(version) = get_string(session, &BCP_PRODUCT_VERSION).await? {
            let version = version
                .strip_prefix('B')
                .map_or(version.clone(), |rest| format!("V{rest}"));
            device.add_firmware(firmware(
                "Toshiba firmware",
                "Toshiba printer firmware",
                version,
            ));
        }
        if let Some(version) = get_string(session, &BCP_DEVICE_BOOT_VERSION).await? {
            device.add_firmware(firmware(
                "Toshiba boot software",
                "Boot software version",
                version,
            ));
        }
        Ok(())
    }
}

/// Builds a Toshiba printer firmware entry.
fn firmware(name: &str, description: &str, version: String) -> Firmware {
    Firmware {
        name: Some(name.to_owned()),
        description: Some(description.to_owned()),
        r#type: Some("printer".to_owned()),
        version: Some(version),
        manufacturer: Some("Toshiba".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::ToshibaMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_toshiba() {
        assert!(ToshibaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1129.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!ToshibaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1130".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_serial_model_and_firmwares() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1129.1.2.1.1.1.1.1.0 = STRING: \"SN12345\"\n\
             .1.3.6.1.4.1.1129.1.2.1.1.1.1.2.0 = STRING: \"B-EX4T1\"\n\
             .1.3.6.1.4.1.1129.1.2.1.1.1.2.1.0 = STRING: \"B-EX4T1-GS12\"\n\
             .1.3.6.1.4.1.1129.1.2.1.1.1.2.5.0 = STRING: \"1.0A\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        ToshibaMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("SN12345"));
        assert_eq!(device.info.model.as_deref(), Some("B-EX4T1-GS12"));
        assert_eq!(device.firmwares.len(), 2);
        // Leading B rewritten to V.
        assert_eq!(device.firmwares[0].version.as_deref(), Some("V-EX4T1"));
        assert_eq!(device.firmwares[1].version.as_deref(), Some("1.0A"));
    }
}
