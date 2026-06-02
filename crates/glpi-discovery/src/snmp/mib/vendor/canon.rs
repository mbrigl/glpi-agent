// SPDX-License-Identifier: GPL-2.0-only

//! Canon printer vendor MIB support.
//!
//! Applies to Canon printers under `1.3.6.1.4.1.1602.4`. Fills the model (from
//! `canPdInfoProductName`, falling back to the Printer-Port-Monitor MIB
//! `ppmPrinterName`), the firmware version and the serial number. Ported from
//! the upstream `GLPI::Agent::SNMP::MibSupport::Canon` (the page counters in
//! the upstream `run` are not modelled here).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Canon printer OID subtree (the upstream module matches `1602.4.*`).
const CANON_PRINTER: &str = "1.3.6.1.4.1.1602.4";
/// `canPdInfoProductName.0` (`canProductInfo.1.0`).
const CAN_PD_INFO_PRODUCT_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 1602, 1, 1, 1, 1, 0];
/// `canPdInfoProductVersion.0` (`canProductInfo.4.0`).
const CAN_PD_INFO_PRODUCT_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 1602, 1, 1, 1, 4, 0];
/// `canServInfoSerialNumberDeviceNumber` (`…1.2.1.8.1.3.1.1`).
const CAN_SERV_INFO_SERIAL: [u64; 15] = [1, 3, 6, 1, 4, 1, 1602, 1, 2, 1, 8, 1, 3, 1, 1];
/// `ppmPrinterName` (`PPM-MIB`, `2699.1.2.1.2.1.1.2.1`).
const PPM_PRINTER_NAME: [u64; 15] = [1, 3, 6, 1, 4, 1, 2699, 1, 2, 1, 2, 1, 1, 2, 1];

/// Vendor MIB module for Canon printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonMib;

#[async_trait]
impl MibSupport for CanonMib {
    fn name(&self) -> &'static str {
        "canon"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), CANON_PRINTER)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = match get_string(session, &CAN_PD_INFO_PRODUCT_NAME).await? {
                Some(name) => Some(name),
                None => get_string(session, &PPM_PRINTER_NAME).await?,
            };
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &CAN_PD_INFO_PRODUCT_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &CAN_SERV_INFO_SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CanonMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_canon_printer_subtree() {
        assert!(CanonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1602.4.7".to_owned()),
            ..DeviceInfo::default()
        }));
        // Canon enterprise but outside the printer subtree.
        assert!(!CanonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1602.1.1".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_firmware_and_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1602.1.1.1.1.0 = STRING: \"iR-ADV C5535\"\n\
             .1.3.6.1.4.1.1602.1.1.1.4.0 = STRING: \"12.05\"\n\
             .1.3.6.1.4.1.1602.1.2.1.8.1.3.1.1 = STRING: \"ABC12345\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        CanonMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("iR-ADV C5535"));
        assert_eq!(device.info.firmware.as_deref(), Some("12.05"));
        assert_eq!(device.info.serial.as_deref(), Some("ABC12345"));
    }

    #[tokio::test]
    async fn falls_back_to_ppm_printer_name() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.2699.1.2.1.2.1.1.2.1 = STRING: \"LBP6030\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        CanonMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("LBP6030"));
    }
}
