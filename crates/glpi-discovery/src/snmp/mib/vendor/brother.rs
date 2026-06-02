// SPDX-License-Identifier: GPL-2.0-only

//! Brother printer vendor MIB support.
//!
//! Applies to devices under the Brother enterprise tree (`1.3.6.1.4.1.2435`)
//! and reads the serial, firmware revision, manufacturer/model (from the
//! NetConfig server description) and the scanned-page counter. Ported from the
//! upstream `GLPI::Agent::SNMP::MibSupport::BrotherNetConfig`; that module is
//! gated by the presence of the `brpsHardwareType` private OID, which our
//! `sysObjectID`-based framework approximates with the Brother enterprise
//! subtree (the NetConfig OIDs simply return nothing on non-Brother gear).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_number, get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Brother enterprise OID (`iso.org.dod.internet.private.enterprises.2435`).
const BROTHER: &str = "1.3.6.1.4.1.2435";
/// `brInfoSerialNumber` (`net-peripheral…printerinformation.1.0`).
const BR_INFO_SERIAL_NUMBER: [u64; 17] = [1, 3, 6, 1, 4, 1, 2435, 2, 3, 9, 4, 2, 1, 5, 5, 1, 0];
/// `brScanCountCounter` (`…printerinformation.54.2.2.1.3.3`).
const BR_SCAN_COUNT_COUNTER: [u64; 21] = [
    1, 3, 6, 1, 4, 1, 2435, 2, 3, 9, 4, 2, 1, 5, 5, 54, 2, 2, 1, 3, 3,
];
/// `brpsMainRevision` (`brnetconfig.brconfig.4.0`).
const BRPS_MAIN_REVISION: [u64; 14] = [1, 3, 6, 1, 4, 1, 2435, 2, 4, 3, 1240, 1, 4, 0];
/// `brpsServerDescription` (`brnetconfig.brconfig.12.0`).
const BRPS_SERVER_DESCRIPTION: [u64; 14] = [1, 3, 6, 1, 4, 1, 2435, 2, 4, 3, 1240, 1, 12, 0];

/// Vendor MIB module for Brother printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct BrotherMib;

#[async_trait]
impl MibSupport for BrotherMib {
    fn name(&self) -> &'static str {
        "brother"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), BROTHER)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &BR_INFO_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &BRPS_MAIN_REVISION).await?;
        }

        // The server description ("Brother <model> …") yields both the
        // manufacturer and, when the model is still unknown, the model name.
        let description = get_string(session, &BRPS_SERVER_DESCRIPTION).await?;
        if let Some(model) = description
            .as_deref()
            .and_then(|d| d.strip_prefix("Brother "))
        {
            if device.info.manufacturer.is_none() {
                device.info.manufacturer = Some("Brother".to_owned());
            }
            if device.info.model.is_none() {
                device.info.model = Some(model.trim().to_owned());
            }
        } else if let Some(model) = device.info.model.as_deref() {
            // A model carried over from another module keeps its value but loses
            // the redundant "Brother " prefix, matching the upstream behaviour.
            if let Some(stripped) = model.strip_prefix("Brother ") {
                device.info.model = Some(stripped.trim().to_owned());
            }
        }

        let scanned = get_number(session, &BR_SCAN_COUNT_COUNTER).await?;
        let counters = &mut device.printer_mut().page_counters;
        counters.scanned = counters.scanned.or(scanned);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BrotherMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_brother_enterprise() {
        assert!(BrotherMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2435.2.3.9.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!BrotherMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2436".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_scanned_counter() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.2435.2.3.9.4.2.1.5.5.1.0 = STRING: \"U63123A1N123456\"\n\
             .1.3.6.1.4.1.2435.2.4.3.1240.1.4.0 = STRING: \"1.42\"\n\
             .1.3.6.1.4.1.2435.2.4.3.1240.1.12.0 = STRING: \"Brother MFC-L8900CDW series\"\n\
             .1.3.6.1.4.1.2435.2.3.9.4.2.1.5.5.54.2.2.1.3.3 = Counter32: 4321\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        BrotherMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("U63123A1N123456"));
        assert_eq!(device.info.firmware.as_deref(), Some("1.42"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Brother"));
        assert_eq!(device.info.model.as_deref(), Some("MFC-L8900CDW series"));
        assert_eq!(device.printer.unwrap().page_counters.scanned, Some(4321));
    }

    #[tokio::test]
    async fn strips_brother_prefix_from_inherited_model() {
        let mut session =
            WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"unrelated\"\n").unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                model: Some("Brother HL-3170CDW".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        BrotherMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("HL-3170CDW"));
    }
}
