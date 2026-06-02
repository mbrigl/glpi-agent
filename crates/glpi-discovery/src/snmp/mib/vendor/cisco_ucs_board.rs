// SPDX-License-Identifier: GPL-2.0-only

//! Cisco UCS board vendor MIB support.
//!
//! Reads the compute-board model and serial from the
//! `CISCO-UNIFIED-COMPUTING-COMPUTE-MIB` (`1.3.6.1.4.1.9.9.719`). Upstream
//! selects this module by the board distinguished-name OID's presence; lacking
//! session access in `applies_to`, we gate on the Cisco enterprise prefix and
//! self-guard in `run` (returning early when the board table is absent). Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::CiscoUcsBoard`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Cisco enterprise OID.
const CISCO: &str = "1.3.6.1.4.1.9";
/// `cucsComputeBoardDn` (`cucsComputeBoardTable.1.2.1`) — presence probe.
const CUCS_COMPUTE_BOARD_DN: [u64; 15] = [1, 3, 6, 1, 4, 1, 9, 9, 719, 1, 9, 6, 1, 2, 1];
/// `cucsComputeBoardModel` (`cucsComputeBoardTable.1.6.1`).
const CUCS_COMPUTE_BOARD_MODEL: [u64; 15] = [1, 3, 6, 1, 4, 1, 9, 9, 719, 1, 9, 6, 1, 6, 1];
/// `cucsComputeBoardSerial` (`cucsComputeBoardTable.1.14.1`).
const CUCS_COMPUTE_BOARD_SERIAL: [u64; 15] = [1, 3, 6, 1, 4, 1, 9, 9, 719, 1, 9, 6, 1, 14, 1];

/// Vendor MIB module for Cisco UCS boards.
#[derive(Debug, Default, Clone, Copy)]
pub struct CiscoUcsBoardMib;

#[async_trait]
impl MibSupport for CiscoUcsBoardMib {
    fn name(&self) -> &'static str {
        "cisco-ucs-board"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), CISCO)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // Only contribute when the compute-board table is present.
        if get_string(session, &CUCS_COMPUTE_BOARD_DN).await?.is_none() {
            return Ok(());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &CUCS_COMPUTE_BOARD_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &CUCS_COMPUTE_BOARD_SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CiscoUcsBoardMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_cisco_enterprise() {
        assert!(CiscoUcsBoardMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.12.3".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!CiscoUcsBoardMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.99".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_board_model_and_serial_when_present() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.9.9.719.1.9.6.1.2.1 = STRING: \"sys/chassis-1/blade-1/board\"\n\
             .1.3.6.1.4.1.9.9.719.1.9.6.1.6.1 = STRING: \"UCSB-B200-M4\"\n\
             .1.3.6.1.4.1.9.9.719.1.9.6.1.14.1 = STRING: \"FCH1900V1AB\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        CiscoUcsBoardMib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.model.as_deref(), Some("UCSB-B200-M4"));
        assert_eq!(device.info.serial.as_deref(), Some("FCH1900V1AB"));
    }

    #[tokio::test]
    async fn no_board_table_leaves_device_untouched() {
        let mut session =
            WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"Cisco IOS\"\n").unwrap();
        let mut device = NetworkDevice::default();
        CiscoUcsBoardMib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.model, None);
        assert_eq!(device.info.serial, None);
    }
}
