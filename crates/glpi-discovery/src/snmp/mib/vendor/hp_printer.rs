// SPDX-License-Identifier: GPL-2.0-only

//! Hewlett-Packard printer vendor MIB support.
//!
//! Applies to HP network peripherals across the several enterprise subtrees the
//! upstream module matches (`hp.nm.system.net-peripheral` `11.2.3.9`, the office
//! printers `29999`, the `11.1` system tree, and the LaserJet Pro MFP `26696.1`).
//! Fills the type/manufacturer, model, serial and firmware — preferring the
//! `gdStatusId` summary string (`MODEL:…;SN:…;FW:…`) when present — and the
//! total/color/duplex page counters. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::HPNetPeripheral`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_number, get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `hp.nm.system.net-peripheral` (`11.2.3.9`).
const HP_PERIPHERAL: &str = "1.3.6.1.4.1.11.2.3.9";
/// HP office printers (`29999`).
const HP_OFFICE_PRINTER: &str = "1.3.6.1.4.1.29999";
/// HP system tree (`11.1`).
const HP_SYSTEM: &str = "1.3.6.1.4.1.11.1";
/// HP LaserJet Pro MFP / Marvel ASIC (`26696.1`).
const HP_LASERJET_PRO_MFP: &str = "1.3.6.1.4.1.26696.1";

/// `gdStatusId` (`net-peripheral.1.1.7.0`) — the `MODEL:…;SN:…;FW:…` summary.
const GD_STATUS_ID: [u64; 14] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 1, 1, 7, 0];
/// `model-name` (`system.id.2.0`).
const MODEL_NAME: [u64; 17] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 4, 2, 1, 1, 3, 2, 0];
/// `serial-number` (`system.id.3.0`).
const SERIAL_NUMBER: [u64; 17] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 4, 2, 1, 1, 3, 3, 0];
/// `fw-rom-revision` (`system.id.6.0`).
const FW_ROM: [u64; 17] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 4, 2, 1, 1, 3, 6, 0];
/// `total-engine-page-count` (`status-prt-eng.5.0`).
const TOTAL_ENGINE_PAGE_COUNT: [u64; 18] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 4, 2, 1, 4, 1, 2, 5, 0];
/// `total-color-page-count` (`status-prt-eng.7.0`).
const TOTAL_COLOR_PAGE_COUNT: [u64; 18] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 4, 2, 1, 4, 1, 2, 7, 0];
/// `duplex-page-count` (`status-prt-eng.22.0`).
const DUPLEX_PAGE_COUNT: [u64; 18] = [1, 3, 6, 1, 4, 1, 11, 2, 3, 9, 4, 2, 1, 4, 1, 2, 22, 0];

/// Vendor MIB module for HP printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct HpPrinterMib;

#[async_trait]
impl MibSupport for HpPrinterMib {
    fn name(&self) -> &'static str {
        "hp-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, HP_PERIPHERAL)
            || sysobjectid_matches(oid, HP_OFFICE_PRINTER)
            || sysobjectid_matches(oid, HP_SYSTEM)
            || sysobjectid_matches(oid, HP_LASERJET_PRO_MFP)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // The status-id string, when present, carries model/serial/firmware as a
        // single `KEY:value;…` summary that is preferred over the scalar OIDs.
        let status = get_string(session, &GD_STATUS_ID).await?;
        let (status_model, status_serial, status_firmware) = status
            .as_deref()
            .map_or((None, None, None), parse_status_id);

        if device.info.r#type.is_none() {
            device.info.r#type = Some("PRINTER".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Hewlett-Packard".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = match status_model {
                Some(model) => Some(model),
                None => get_string(session, &MODEL_NAME).await?,
            };
        }
        if device.info.serial.is_none() {
            device.info.serial = match status_serial {
                Some(serial) => Some(serial),
                None => get_string(session, &SERIAL_NUMBER).await?,
            };
        }
        if device.info.firmware.is_none() {
            device.info.firmware = match status_firmware {
                Some(firmware) => Some(firmware),
                None => get_string(session, &FW_ROM).await?,
            };
        }

        // Page counters: only set what the standard Printer-MIB did not provide.
        let counters = &mut device.printer_mut().page_counters;
        if counters.total.is_none() {
            counters.total = get_number(session, &TOTAL_ENGINE_PAGE_COUNT).await?;
        }
        if counters.color.is_none() {
            counters.color = get_number(session, &TOTAL_COLOR_PAGE_COUNT).await?;
        }
        if counters.rectoverso.is_none() {
            counters.rectoverso = get_number(session, &DUPLEX_PAGE_COUNT).await?;
        }
        Ok(())
    }
}

/// Parses the `gdStatusId` summary (`MODEL:…;SN:…;FW:…`) into the
/// `(model, serial, firmware)` fields it advertises, ignoring unknown keys.
fn parse_status_id(status: &str) -> (Option<String>, Option<String>, Option<String>) {
    let (mut model, mut serial, mut firmware) = (None, None, None);
    for token in status.split(';') {
        let Some((key, value)) = token.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "MODEL" => model = Some(value.to_owned()),
            "SN" => serial = Some(value.to_owned()),
            "FW" => firmware = Some(value.to_owned()),
            _ => {}
        }
    }
    (model, serial, firmware)
}

#[cfg(test)]
mod tests {
    use super::HpPrinterMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_each_hp_subtree() {
        for oid in [
            "1.3.6.1.4.1.11.2.3.9.1",
            "1.3.6.1.4.1.29999.1",
            "1.3.6.1.4.1.11.1.4",
            "1.3.6.1.4.1.26696.1.1",
        ] {
            assert!(HpPrinterMib.applies_to(&DeviceInfo {
                sys_object_id: Some(oid.to_owned()),
                ..DeviceInfo::default()
            }));
        }
        assert!(!HpPrinterMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn prefers_status_id_summary() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.11.2.3.9.1.1.7.0 = STRING: \"MODEL:HP LaserJet M607;SN:VNB1234567;FW:2.5.1\"\n\
             .1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.2.0 = STRING: \"ignored-model\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HpPrinterMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("PRINTER"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Hewlett-Packard"));
        assert_eq!(device.info.model.as_deref(), Some("HP LaserJet M607"));
        assert_eq!(device.info.serial.as_deref(), Some("VNB1234567"));
        assert_eq!(device.info.firmware.as_deref(), Some("2.5.1"));
    }

    #[tokio::test]
    async fn falls_back_to_scalar_oids_and_reads_counters() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.2.0 = STRING: \"HP Color LaserJet\"\n\
             .1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.3.0 = STRING: \"CNX9988\"\n\
             .1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.6.0 = STRING: \"3.1.0\"\n\
             .1.3.6.1.4.1.11.2.3.9.4.2.1.4.1.2.5.0 = Counter32: 50000\n\
             .1.3.6.1.4.1.11.2.3.9.4.2.1.4.1.2.7.0 = Counter32: 12000\n\
             .1.3.6.1.4.1.11.2.3.9.4.2.1.4.1.2.22.0 = Counter32: 8000\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HpPrinterMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("HP Color LaserJet"));
        assert_eq!(device.info.serial.as_deref(), Some("CNX9988"));
        assert_eq!(device.info.firmware.as_deref(), Some("3.1.0"));
        let c = &device.printer.unwrap().page_counters;
        assert_eq!(c.total, Some(50000));
        assert_eq!(c.color, Some(12000));
        assert_eq!(c.rectoverso, Some(8000));
    }
}
