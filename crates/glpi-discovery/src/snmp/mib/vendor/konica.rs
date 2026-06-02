// SPDX-License-Identifier: GPL-2.0-only

//! Konica Minolta (and Sindoh) printer vendor MIB support.
//!
//! Konica Minolta and Sindoh printers share the same private MIB
//! (`1.3.6.1.4.1.18334`); the upstream module matches both sysObjectID prefixes
//! (`konica.1.1.1.2` and `konica.1.2.1.2`). Reads the model, the per-type page
//! counters and the firmware inventory. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Konica`.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_number, get_string, instance_suffix, sysobjectid_matches, DeviceInfo, Firmware, MibSupport,
    NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `konicaSysobjectID` (`konica.1.1.1.2`) — Konica Minolta printers.
const KONICA_SYSOBJECTID: &str = "1.3.6.1.4.1.18334.1.1.1.2";
/// `sindohSysobjectID` (`konica.1.2.1.2`) — Sindoh printers (same MIB).
const SINDOH_SYSOBJECTID: &str = "1.3.6.1.4.1.18334.1.2.1.2";

/// `konicaModel` (`konica.1.1.1.1.6.2.1.0`).
const KONICA_MODEL: [u64; 15] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 1, 6, 2, 1, 0];

// Page counters under `konicaPrinterCounters` (`konica.1.1.1.5.7.2`).
/// `konicaTotal` (`…5.7.2.1.1.0`).
const KONICA_TOTAL: [u64; 16] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 1, 1, 0];
/// `konicaRectoVerso` (`…5.7.2.1.3.0`).
const KONICA_RECTOVERSO: [u64; 16] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 1, 3, 0];
/// `konicaBlackCopy` (`…5.7.2.2.1.5.1.1`).
const KONICA_BLACK_COPY: [u64; 18] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 2, 1, 5, 1, 1];
/// `konicaBlackPrint` (`…5.7.2.2.1.5.1.2`).
const KONICA_BLACK_PRINT: [u64; 18] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 2, 1, 5, 1, 2];
/// `konicaColorCopy` (`…5.7.2.2.1.5.2.1`).
const KONICA_COLOR_COPY: [u64; 18] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 2, 1, 5, 2, 1];
/// `konicaColorPrint` (`…5.7.2.2.1.5.2.2`).
const KONICA_COLOR_PRINT: [u64; 18] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 2, 1, 5, 2, 2];
/// `konicaScans` (`…5.7.2.3.1.5.1`).
const KONICA_SCANS: [u64; 17] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 7, 2, 3, 1, 5, 1];

/// `konicaFirmwareName` (`konica.1.1.1.5.5.1.1.2`).
const KONICA_FIRMWARE_NAME: [u64; 15] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 5, 1, 1, 2];
/// `konicaFirmwareVersion` (`konica.1.1.1.5.5.1.1.3`).
const KONICA_FIRMWARE_VERSION: [u64; 15] = [1, 3, 6, 1, 4, 1, 18334, 1, 1, 1, 5, 5, 1, 1, 3];

/// Vendor MIB module for Konica Minolta / Sindoh printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct KonicaMib;

#[async_trait]
impl MibSupport for KonicaMib {
    fn name(&self) -> &'static str {
        "konica-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, KONICA_SYSOBJECTID) || sysobjectid_matches(oid, SINDOH_SYSOBJECTID)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            if let Some(model) = get_string(session, &KONICA_MODEL).await? {
                // Strip the redundant "KONICA MINOLTA " manufacturer prefix.
                let model = model
                    .strip_prefix("KONICA MINOLTA ")
                    .or_else(|| model.strip_prefix("Konica Minolta "))
                    .unwrap_or(&model)
                    .to_owned();
                device.info.model = Some(model);
            }
        }

        let total = get_number(session, &KONICA_TOTAL).await?;
        let rectoverso = get_number(session, &KONICA_RECTOVERSO).await?;
        let copy_black = get_number(session, &KONICA_BLACK_COPY).await?;
        let print_black = get_number(session, &KONICA_BLACK_PRINT).await?;
        let copy_color = get_number(session, &KONICA_COLOR_COPY).await?;
        let print_color = get_number(session, &KONICA_COLOR_PRINT).await?;
        let scanned = get_number(session, &KONICA_SCANS).await?;

        let counters = &mut device.printer_mut().page_counters;
        counters.total = counters.total.or(total);
        counters.rectoverso = counters.rectoverso.or(rectoverso);
        counters.copy_black = counters.copy_black.or(copy_black);
        counters.print_black = counters.print_black.or(print_black);
        counters.copy_color = counters.copy_color.or(copy_color);
        counters.print_color = counters.print_color.or(print_color);
        counters.scanned = counters.scanned.or(scanned);

        // Derive the print/copy totals from their black + colour parts when the
        // device exposes no dedicated total counter.
        if counters.print_total.is_none() && (print_color.is_some() || print_black.is_some()) {
            counters.print_total = Some(print_black.unwrap_or(0) + print_color.unwrap_or(0));
        }
        if counters.copy_total.is_none() && (copy_color.is_some() || copy_black.is_some()) {
            counters.copy_total = Some(copy_black.unwrap_or(0) + copy_color.unwrap_or(0));
        }

        self.read_firmwares(session, device).await
    }
}

impl KonicaMib {
    /// Walks the Konica firmware table and records each named entry, skipping
    /// the placeholder versions (`-`, `Registered`) the device reports for
    /// unpopulated slots.
    async fn read_firmwares(
        &self,
        session: &mut dyn SnmpQuery,
        device: &mut NetworkDevice,
    ) -> Result<()> {
        let mut names: BTreeMap<Vec<u64>, String> = BTreeMap::new();
        for (oid, value) in session.walk(&KONICA_FIRMWARE_NAME).await? {
            if let (Some(suffix), Some(name)) = (
                instance_suffix(&oid, &KONICA_FIRMWARE_NAME),
                value.as_str().filter(|s| !s.is_empty()),
            ) {
                names.insert(suffix, name);
            }
        }
        let mut versions: BTreeMap<Vec<u64>, String> = BTreeMap::new();
        for (oid, value) in session.walk(&KONICA_FIRMWARE_VERSION).await? {
            if let (Some(suffix), Some(version)) = (
                instance_suffix(&oid, &KONICA_FIRMWARE_VERSION),
                value.as_str().filter(|s| !s.is_empty()),
            ) {
                versions.insert(suffix, version);
            }
        }

        for (index, name) in names {
            let Some(version) = versions.get(&index) else {
                continue;
            };
            if version == "-" || version == "Registered" {
                continue;
            }
            // Strip a trailing " version" from the firmware name.
            let name = name
                .strip_suffix(" version")
                .or_else(|| name.strip_suffix(" Version"))
                .unwrap_or(&name)
                .to_owned();
            device.add_firmware(Firmware {
                name: Some(format!("Konica {name}")),
                description: Some(format!("Printer {name}")),
                r#type: Some("printer".to_owned()),
                version: Some(version.clone()),
                manufacturer: Some("Konica".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::KonicaMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_konica_and_sindoh() {
        assert!(KonicaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.18334.1.1.1.2.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(KonicaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.18334.1.2.1.2.5".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!KonicaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.18334.2".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_model_counters_and_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.18334.1.1.1.1.6.2.1.0 = STRING: \"KONICA MINOLTA bizhub C284e\"\n\
             .1.3.6.1.4.1.18334.1.1.1.5.7.2.1.1.0 = Counter32: 250000\n\
             .1.3.6.1.4.1.18334.1.1.1.5.7.2.2.1.5.1.1 = Counter32: 1000\n\
             .1.3.6.1.4.1.18334.1.1.1.5.7.2.2.1.5.1.2 = Counter32: 2000\n\
             .1.3.6.1.4.1.18334.1.1.1.5.7.2.2.1.5.2.1 = Counter32: 3000\n\
             .1.3.6.1.4.1.18334.1.1.1.5.7.2.2.1.5.2.2 = Counter32: 4000\n\
             .1.3.6.1.4.1.18334.1.1.1.5.7.2.3.1.5.1 = Counter32: 500\n\
             .1.3.6.1.4.1.18334.1.1.1.5.5.1.1.2.1 = STRING: \"Controller version\"\n\
             .1.3.6.1.4.1.18334.1.1.1.5.5.1.1.3.1 = STRING: \"A1.2.3\"\n\
             .1.3.6.1.4.1.18334.1.1.1.5.5.1.1.2.2 = STRING: \"Engine\"\n\
             .1.3.6.1.4.1.18334.1.1.1.5.5.1.1.3.2 = STRING: \"-\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        KonicaMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("bizhub C284e"));
        let c = &device.printer.as_ref().unwrap().page_counters;
        assert_eq!(c.total, Some(250000));
        assert_eq!(c.copy_black, Some(1000));
        assert_eq!(c.print_black, Some(2000));
        assert_eq!(c.copy_color, Some(3000));
        assert_eq!(c.print_color, Some(4000));
        assert_eq!(c.scanned, Some(500));
        assert_eq!(c.print_total, Some(6000)); // 2000 + 4000
        assert_eq!(c.copy_total, Some(4000)); // 1000 + 3000

        // Only the populated firmware row is recorded; the "-" version is skipped.
        assert_eq!(device.firmwares.len(), 1);
        let fw = &device.firmwares[0];
        assert_eq!(fw.name.as_deref(), Some("Konica Controller"));
        assert_eq!(fw.version.as_deref(), Some("A1.2.3"));
        assert_eq!(fw.manufacturer.as_deref(), Some("Konica"));
    }
}
