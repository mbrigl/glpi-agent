// SPDX-License-Identifier: GPL-2.0-only

//! HPE iLO (Integrated Lights-Out) vendor MIB support.
//!
//! Applies to Compaq/HPE iLO controllers, whose `sysObjectID` starts with
//! `1.3.6.1.4.1.232.9.4`, and fills the firmware and serial from the
//! `cpqSm2Cntrl` group. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::iLO`; the iLO NIC port/MAC/IP details are not
//! modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// The iLO `sysObjectID` prefix (`compaq.9.4`).
const ILO_SYSOBJECTID: &str = "1.3.6.1.4.1.232.9.4";
/// `cpqSm2CntrlFirmwareRevision` (`cpqSm2Cntrl.2.0`).
const FIRMWARE: [u64; 12] = [1, 3, 6, 1, 4, 1, 232, 9, 2, 2, 2, 0];
/// `cpqSm2CntrlSerialNumber` (`cpqSm2Cntrl.15.0`).
const SERIAL: [u64; 12] = [1, 3, 6, 1, 4, 1, 232, 9, 2, 2, 15, 0];

/// Vendor MIB module for HPE iLO controllers.
#[derive(Debug, Default, Clone, Copy)]
pub struct IloMib;

#[async_trait]
impl MibSupport for IloMib {
    fn name(&self) -> &'static str {
        "cpqsm2"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), ILO_SYSOBJECTID)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &FIRMWARE).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::IloMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_ilo_sysobjectid() {
        assert!(IloMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.232.9.4.10".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!IloMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.232.9.5".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_firmware_and_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.232.9.2.2.2.0 = STRING: \"2.78\"\n\
             .1.3.6.1.4.1.232.9.2.2.15.0 = STRING: \"ILOABC1234567\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        IloMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.firmware.as_deref(), Some("2.78"));
        assert_eq!(device.info.serial.as_deref(), Some("ILOABC1234567"));
    }
}
