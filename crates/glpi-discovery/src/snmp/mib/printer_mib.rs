// SPDX-License-Identifier: GPL-2.0-only

//! Standard `Printer-MIB` support (RFC 3805).
//!
//! Reads the lifetime page count (`prtMarkerLifeCount`) and the consumables
//! table (`prtMarkerSuppliesTable`: description, type, level, capacity, unit)
//! into a [`Printer`]. Runs for every device but only attaches a printer record
//! when something printer-specific is actually present, so it is a no-op on
//! switches and routers.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use super::{apply_suffix_column, as_number, MibSupport, NetworkDevice, Printer, Supply};
use crate::snmp::query::SnmpQuery;

/// `prtMarkerLifeCount` (indexed by `hrDeviceIndex.markerIndex`).
const PRT_MARKER_LIFE_COUNT: [u64; 11] = [1, 3, 6, 1, 2, 1, 43, 10, 2, 1, 4];
/// `prtGeneralSerialNumber` (indexed by `hrDeviceIndex`).
const PRT_GENERAL_SERIAL_NUMBER: [u64; 11] = [1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 17];
// prtMarkerSuppliesTable columns (1.3.6.1.2.1.43.11.1.1.N).
const PRT_SUPPLIES_TYPE: [u64; 10] = [1, 3, 6, 1, 2, 1, 43, 11, 1, 1];
const SUPPLIES_TYPE_COL: u64 = 5;
const SUPPLIES_DESCRIPTION_COL: u64 = 6;
const SUPPLIES_UNIT_COL: u64 = 7;
const SUPPLIES_MAX_CAPACITY_COL: u64 = 8;
const SUPPLIES_LEVEL_COL: u64 = 9;

/// MIB module for the standard printer tables.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrinterMib;

#[async_trait]
impl MibSupport for PrinterMib {
    fn name(&self) -> &'static str {
        "printer"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // Lifetime page count: take the largest across markers.
        let total_pages = session
            .walk(&PRT_MARKER_LIFE_COUNT)
            .await?
            .into_iter()
            .filter_map(|(_, value)| as_number(&value))
            .max();

        let supplies = read_supplies(session).await?;

        if total_pages.is_some() || !supplies.is_empty() {
            device.printer = Some(Printer {
                total_pages,
                supplies,
                ..Printer::default()
            });
        }

        // The standard printer serial applies to every brand; only set it if a
        // prior module (ENTITY-MIB / vendor) did not already.
        if device.info.serial.is_none() {
            if let Some((_, value)) = session
                .walk(&PRT_GENERAL_SERIAL_NUMBER)
                .await?
                .into_iter()
                .next()
            {
                device.info.serial = value.as_str().filter(|s| !s.is_empty());
            }
        }
        Ok(())
    }
}

/// Walks `prtMarkerSuppliesTable` into the supplies list, ordered by index.
async fn read_supplies(session: &mut dyn SnmpQuery) -> Result<Vec<Supply>> {
    let mut supplies: BTreeMap<Vec<u64>, Supply> = BTreeMap::new();
    let new = |_suffix: &[u64]| Supply::default();

    apply_suffix_column(
        session,
        &column(SUPPLIES_TYPE_COL),
        &mut supplies,
        new,
        |s, v| {
            s.r#type = as_number(&v);
        },
    )
    .await?;
    apply_suffix_column(
        session,
        &column(SUPPLIES_DESCRIPTION_COL),
        &mut supplies,
        new,
        |s, v| s.description = v.as_str(),
    )
    .await?;
    apply_suffix_column(
        session,
        &column(SUPPLIES_UNIT_COL),
        &mut supplies,
        new,
        |s, v| {
            s.unit = as_number(&v);
        },
    )
    .await?;
    apply_suffix_column(
        session,
        &column(SUPPLIES_MAX_CAPACITY_COL),
        &mut supplies,
        new,
        |s, v| s.max_capacity = as_number(&v),
    )
    .await?;
    apply_suffix_column(
        session,
        &column(SUPPLIES_LEVEL_COL),
        &mut supplies,
        new,
        |s, v| s.level = as_number(&v),
    )
    .await?;

    Ok(supplies.into_values().collect())
}

/// Builds the OID of `prtMarkerSuppliesTable` column `col`.
fn column(col: u64) -> Vec<u64> {
    let mut oid = PRT_SUPPLIES_TYPE.to_vec();
    oid.push(col);
    oid
}

#[cfg(test)]
mod tests {
    use super::PrinterMib;
    use crate::snmp::mib::{MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    const PRINTER_WALK: &str = r#".1.3.6.1.2.1.43.10.2.1.4.1.1 = Counter32: 45210
.1.3.6.1.2.1.43.5.1.1.17.1 = STRING: "PRN-SN-001"
.1.3.6.1.2.1.43.11.1.1.5.1.1 = INTEGER: 3
.1.3.6.1.2.1.43.11.1.1.6.1.1 = STRING: "Black Toner Cartridge"
.1.3.6.1.2.1.43.11.1.1.7.1.1 = INTEGER: 19
.1.3.6.1.2.1.43.11.1.1.8.1.1 = INTEGER: 100
.1.3.6.1.2.1.43.11.1.1.9.1.1 = INTEGER: 80
.1.3.6.1.2.1.43.11.1.1.6.1.2 = STRING: "Cyan Toner Cartridge"
.1.3.6.1.2.1.43.11.1.1.9.1.2 = INTEGER: 50
"#;

    #[tokio::test]
    async fn reads_page_count_and_supplies() {
        let mut session = WalkSession::parse(PRINTER_WALK).unwrap();
        let mut device = NetworkDevice::default();
        PrinterMib.run(&mut session, &mut device).await.unwrap();

        assert_eq!(device.info.serial.as_deref(), Some("PRN-SN-001"));

        let printer = device.printer.expect("printer record");
        assert_eq!(printer.total_pages, Some(45210));
        assert_eq!(printer.supplies.len(), 2);

        let black = &printer.supplies[0];
        assert_eq!(black.description.as_deref(), Some("Black Toner Cartridge"));
        assert_eq!(black.r#type, Some(3));
        assert_eq!(black.level, Some(80));
        assert_eq!(black.max_capacity, Some(100));
        assert_eq!(black.unit, Some(19));

        let cyan = &printer.supplies[1];
        assert_eq!(cyan.description.as_deref(), Some("Cyan Toner Cartridge"));
        assert_eq!(cyan.level, Some(50));
    }

    #[tokio::test]
    async fn non_printer_gets_no_printer_record() {
        let mut session =
            WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"a switch\"\n").unwrap();
        let mut device = NetworkDevice::default();
        PrinterMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.printer, None);
    }
}
