// SPDX-License-Identifier: GPL-2.0-only

//! Pantum printer vendor MIB support.
//!
//! Applies to Pantum printers (`1.3.6.1.4.1.40093.1.1`). Sets the manufacturer,
//! fills the model from the standard `Printer-MIB` printer name, and resolves
//! the serial number from the first of three Pantum-specific OIDs that answers.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Pantum` (the
//! supply/counter enumeration is not modelled here).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Pantum `pantumPrinter` OID (`pantum.1.1`).
const PANTUM_PRINTER: &str = "1.3.6.1.4.1.40093.1.1";
/// `prtGeneralPrinterName.1` (standard `Printer-MIB`).
const PRT_GENERAL_PRINTER_NAME: [u64; 12] = [1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 16, 1];
/// `pantumSerialNumber1` (`pantumPrinter.1.5`).
const PANTUM_SERIAL_1: [u64; 11] = [1, 3, 6, 1, 4, 1, 40093, 1, 1, 1, 5];
/// `pantumSerialNumber2` (`pantum.6.1.2`).
const PANTUM_SERIAL_2: [u64; 10] = [1, 3, 6, 1, 4, 1, 40093, 6, 1, 2];
/// `pantumSerialNumber3` (`pantum.10.1.1.4`).
const PANTUM_SERIAL_3: [u64; 11] = [1, 3, 6, 1, 4, 1, 40093, 10, 1, 1, 4];

/// Vendor MIB module for Pantum printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct PantumMib;

#[async_trait]
impl MibSupport for PantumMib {
    fn name(&self) -> &'static str {
        "pantum-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), PANTUM_PRINTER)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Pantum".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PRT_GENERAL_PRINTER_NAME).await?;
        }
        if device.info.serial.is_none() {
            for oid in [
                PANTUM_SERIAL_1.as_slice(),
                PANTUM_SERIAL_2.as_slice(),
                PANTUM_SERIAL_3.as_slice(),
            ] {
                if let Some(serial) = get_string(session, oid).await? {
                    device.info.serial = Some(serial);
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PantumMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_pantum() {
        assert!(PantumMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.40093.1.1.5".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!PantumMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_manufacturer_model_and_falls_back_for_serial() {
        // Only the third serial OID answers.
        let mut session = WalkSession::parse(
            ".1.3.6.1.2.1.43.5.1.1.16.1 = STRING: \"BM5100ADN\"\n\
             .1.3.6.1.4.1.40093.10.1.1.4 = STRING: \"PT12345ABCDE\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        PantumMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Pantum"));
        assert_eq!(device.info.model.as_deref(), Some("BM5100ADN"));
        assert_eq!(device.info.serial.as_deref(), Some("PT12345ABCDE"));
    }
}
