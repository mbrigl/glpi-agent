// SPDX-License-Identifier: GPL-2.0-only

//! Ricoh printer vendor MIB support.
//!
//! Applies to Ricoh printers (matched on `ricohAgentsID`,
//! `1.3.6.1.4.1.367.1.1`). Reads the model from the standard Printer-MIB
//! `prtGeneralPrinterName` and maps the Ricoh engine-counter table
//! (`ricohEngCounter`) into the device's [`PageCounters`]. Ported from the
//! upstream `GLPI::Agent::SNMP::MibSupport::Ricoh`.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    as_number, get_string, sysobjectid_matches, table_index, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `ricohAgentsID` (`ricoh.1.1`) — the sysObjectID prefix Ricoh printers use.
const RICOH_AGENTS_ID: &str = "1.3.6.1.4.1.367.1.1";
/// `prtGeneralPrinterName` (Printer-MIB `43.5.1.1.16.1`).
const PRT_GENERAL_PRINTER_NAME: [u64; 12] = [1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 16, 1];
/// `ricohEngCounterType` (`ricohEngCounter.5.1.2`) — counter category per row.
const RICOH_ENG_COUNTER_TYPE: [u64; 15] = [1, 3, 6, 1, 4, 1, 367, 3, 2, 1, 2, 19, 5, 1, 2];
/// `ricohEngCounterValue` (`ricohEngCounter.5.1.9`) — counter value per row.
const RICOH_ENG_COUNTER_VALUE: [u64; 15] = [1, 3, 6, 1, 4, 1, 367, 3, 2, 1, 2, 19, 5, 1, 9];

/// Vendor MIB module for Ricoh printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct RicohMib;

/// Maps a `ricohEngCounterType` value to a [`PageCounters`] field and whether
/// several rows accumulate into it (the colour and scan categories are split
/// across two type codes each).
fn counter_for(type_code: i64) -> Option<(Counter, bool)> {
    let entry = match type_code {
        10 => (Counter::Total, false),
        200 => (Counter::CopyTotal, false),
        201 => (Counter::CopyBlack, false),
        202 | 203 => (Counter::CopyColor, true),
        300 => (Counter::FaxTotal, false),
        400 => (Counter::PrintTotal, false),
        401 => (Counter::PrintBlack, false),
        402 | 403 => (Counter::PrintColor, true),
        870 | 871 => (Counter::Scanned, true),
        _ => return None,
    };
    Some(entry)
}

/// The subset of [`PageCounters`] fields the Ricoh engine counters populate.
#[derive(Clone, Copy)]
enum Counter {
    Total,
    CopyTotal,
    CopyBlack,
    CopyColor,
    FaxTotal,
    PrintTotal,
    PrintBlack,
    PrintColor,
    Scanned,
}

#[async_trait]
impl MibSupport for RicohMib {
    fn name(&self) -> &'static str {
        "ricoh-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), RICOH_AGENTS_ID)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PRT_GENERAL_PRINTER_NAME).await?;
        }

        // Row index → counter type, and the same index → value.
        let mut types: BTreeMap<u64, i64> = BTreeMap::new();
        for (oid, value) in session.walk(&RICOH_ENG_COUNTER_TYPE).await? {
            if let (Some(index), Some(type_code)) = (
                table_index(&oid, &RICOH_ENG_COUNTER_TYPE),
                as_number(&value),
            ) {
                types.insert(index, type_code);
            }
        }
        if types.is_empty() {
            return Ok(());
        }
        let mut values: BTreeMap<u64, i64> = BTreeMap::new();
        for (oid, value) in session.walk(&RICOH_ENG_COUNTER_VALUE).await? {
            if let (Some(index), Some(count)) = (
                table_index(&oid, &RICOH_ENG_COUNTER_VALUE),
                as_number(&value),
            ) {
                values.insert(index, count);
            }
        }

        let counters = &mut device.printer_mut().page_counters;
        for (index, type_code) in types {
            let Some((field, accumulate)) = counter_for(type_code) else {
                continue;
            };
            let Some(&count) = values.get(&index) else {
                continue;
            };
            let slot = match field {
                Counter::Total => &mut counters.total,
                Counter::CopyTotal => &mut counters.copy_total,
                Counter::CopyBlack => &mut counters.copy_black,
                Counter::CopyColor => &mut counters.copy_color,
                Counter::FaxTotal => &mut counters.fax_total,
                Counter::PrintTotal => &mut counters.print_total,
                Counter::PrintBlack => &mut counters.print_black,
                Counter::PrintColor => &mut counters.print_color,
                Counter::Scanned => &mut counters.scanned,
            };
            *slot = if accumulate {
                Some(slot.unwrap_or(0) + count)
            } else {
                Some(count)
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RicohMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_ricoh_agents_id() {
        assert!(RicohMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.367.1.1.17".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RicohMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.367.3.2".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn maps_and_accumulates_engine_counters() {
        // Two colour-print rows (402 + 403) accumulate; total/black are single.
        let mut session = WalkSession::parse(
            ".1.3.6.1.2.1.43.5.1.1.16.1 = STRING: \"MP C3004\"\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.2.1 = INTEGER: 10\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.2.2 = INTEGER: 401\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.2.3 = INTEGER: 402\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.2.4 = INTEGER: 403\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.9.1 = Counter32: 90000\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.9.2 = Counter32: 60000\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.9.3 = Counter32: 20000\n\
             .1.3.6.1.4.1.367.3.2.1.2.19.5.1.9.4 = Counter32: 10000\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        RicohMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("MP C3004"));
        let c = &device.printer.unwrap().page_counters;
        assert_eq!(c.total, Some(90000));
        assert_eq!(c.print_black, Some(60000));
        assert_eq!(c.print_color, Some(30000)); // 20000 + 10000
    }
}
