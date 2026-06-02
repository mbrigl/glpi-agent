// SPDX-License-Identifier: GPL-2.0-only

//! Brocade switch vendor MIB support.
//!
//! Applies to Brocade switches (`1.3.6.1.4.1.1991`) and fills the serial and the
//! primary firmware version. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Brocade`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Brocade enterprise OID.
const BROCADE: &str = "1.3.6.1.4.1.1991";
/// `serial` (`brocade.1.1.1.1.2.0`).
const SERIAL: [u64; 13] = [1, 3, 6, 1, 4, 1, 1991, 1, 1, 1, 1, 2, 0];
/// `fw_pri` (`brocade.1.1.2.1.11.0`).
const FW_PRI: [u64; 13] = [1, 3, 6, 1, 4, 1, 1991, 1, 1, 2, 1, 11, 0];

/// Vendor MIB module for Brocade switches.
#[derive(Debug, Default, Clone, Copy)]
pub struct BrocadeMib;

#[async_trait]
impl MibSupport for BrocadeMib {
    fn name(&self) -> &'static str {
        "brocade-switch"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), BROCADE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &FW_PRI).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BrocadeMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_brocade() {
        assert!(BrocadeMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1991.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!BrocadeMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1992".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_serial_and_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1991.1.1.1.1.2.0 = STRING: \"BKB2540F00X\"\n\
             .1.3.6.1.4.1.1991.1.1.2.1.11.0 = STRING: \"08.0.30hT311\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        BrocadeMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("BKB2540F00X"));
        assert_eq!(device.info.firmware.as_deref(), Some("08.0.30hT311"));
    }
}
