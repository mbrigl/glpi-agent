// SPDX-License-Identifier: GPL-2.0-only

//! Xerox printer vendor MIB support.
//!
//! Applies to Xerox printers (`XEROX-COMMON-MIB`, `1.3.6.1.4.1.253.8`) and
//! reads the per-type page counters from `xcmHrDevDetailEntry` into the
//! device's [`PageCounters`]. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Xerox`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_number, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `xeroxCommonMIB` OID (`xerox.8`).
const XEROX_COMMON_MIB: &str = "1.3.6.1.4.1.253.8";
/// `xeroxTotalPrint` (`xcmHrDevDetailEntry.6.1.20.1`).
const XEROX_TOTAL_PRINT: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 1, 20, 1];
/// `xeroxColorPrint` (`…6.1.20.33`).
const XEROX_COLOR_PRINT: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 1, 20, 33];
/// `xeroxBlackPrint` (`…6.1.20.34`).
const XEROX_BLACK_PRINT: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 1, 20, 34];
/// `xeroxColorCopy` (`…6.11.20.25`).
const XEROX_COLOR_COPY: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 11, 20, 25];
/// `xeroxBlackCopy` (`…6.11.20.3`).
const XEROX_BLACK_COPY: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 11, 20, 3];
/// `xeroxScanSentByEmail` (`…6.10.20.11`).
const XEROX_SCAN_EMAIL: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 10, 20, 11];
/// `xeroxScanSavedOnNetwork` (`…6.10.20.12`).
const XEROX_SCAN_NETWORK: [u64; 16] = [1, 3, 6, 1, 4, 1, 253, 8, 53, 13, 2, 1, 6, 10, 20, 12];

/// Vendor MIB module for Xerox printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct XeroxMib;

#[async_trait]
impl MibSupport for XeroxMib {
    fn name(&self) -> &'static str {
        "xerox-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), XEROX_COMMON_MIB)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let print_total = get_number(session, &XEROX_TOTAL_PRINT).await?;
        let print_color = get_number(session, &XEROX_COLOR_PRINT).await?;
        let print_black = get_number(session, &XEROX_BLACK_PRINT).await?;
        let copy_color = get_number(session, &XEROX_COLOR_COPY).await?;
        let copy_black = get_number(session, &XEROX_BLACK_COPY).await?;
        // SCANNED is the sum of the e-mail and network destinations.
        let scan_email = get_number(session, &XEROX_SCAN_EMAIL).await?;
        let scan_network = get_number(session, &XEROX_SCAN_NETWORK).await?;
        let scanned = match (scan_email, scan_network) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
        };
        // COPYTOTAL is derived from the black/color copy counts.
        let copy_total = match (copy_black, copy_color) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
        };

        let counters = &mut device.printer_mut().page_counters;
        counters.print_total = counters.print_total.or(print_total);
        counters.print_color = counters.print_color.or(print_color);
        counters.print_black = counters.print_black.or(print_black);
        counters.copy_color = counters.copy_color.or(copy_color);
        counters.copy_black = counters.copy_black.or(copy_black);
        counters.copy_total = counters.copy_total.or(copy_total);
        counters.scanned = counters.scanned.or(scanned);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::XeroxMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_xerox() {
        assert!(XeroxMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.253.8.62".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!XeroxMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_and_derives_page_counters() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.253.8.53.13.2.1.6.1.20.1 = Counter32: 100000\n\
             .1.3.6.1.4.1.253.8.53.13.2.1.6.1.20.33 = Counter32: 40000\n\
             .1.3.6.1.4.1.253.8.53.13.2.1.6.1.20.34 = Counter32: 60000\n\
             .1.3.6.1.4.1.253.8.53.13.2.1.6.11.20.25 = Counter32: 1500\n\
             .1.3.6.1.4.1.253.8.53.13.2.1.6.11.20.3 = Counter32: 2500\n\
             .1.3.6.1.4.1.253.8.53.13.2.1.6.10.20.11 = Counter32: 700\n\
             .1.3.6.1.4.1.253.8.53.13.2.1.6.10.20.12 = Counter32: 300\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        XeroxMib.run(&mut session, &mut device).await.unwrap();
        let c = &device.printer.unwrap().page_counters;
        assert_eq!(c.print_total, Some(100000));
        assert_eq!(c.print_color, Some(40000));
        assert_eq!(c.print_black, Some(60000));
        assert_eq!(c.copy_color, Some(1500));
        assert_eq!(c.copy_black, Some(2500));
        assert_eq!(c.copy_total, Some(4000));
        assert_eq!(c.scanned, Some(1000));
    }
}
