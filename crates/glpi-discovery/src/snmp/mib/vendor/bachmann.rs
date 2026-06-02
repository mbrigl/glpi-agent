// SPDX-License-Identifier: GPL-2.0-only

//! Bachmann PDU vendor MIB support.
//!
//! Applies to Bachmann power-distribution units (`NETTRACK-E3METER-SNMP-MIB`
//! `public`, `1.3.6.1.4.1.21695.1`). Fills the manufacturer and serial, decodes
//! the packed firmware revision (`major*256 + minor`) and records the hardware
//! revision as a firmware entry. The device type is reported as `NETWORKING`
//! (the upstream `PDU` type needs GLPI 12, which is not modelled here). Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::Bachmann`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_number, get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `public` (`nettrack.1`) — the Bachmann e3meter OID subtree.
const PUBLIC: &str = "1.3.6.1.4.1.21695.1";
/// `e3IpmInfoSerial` (`e3Ipm.1.1`).
const E3_IPM_INFO_SERIAL: [u64; 12] = [1, 3, 6, 1, 4, 1, 21695, 1, 10, 7, 1, 1];
/// `e3IpmInfoHWVersion` (`e3Ipm.1.3`).
const E3_IPM_INFO_HW_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 21695, 1, 10, 7, 1, 3];
/// `e3IpmInfoFWVersion` (`e3Ipm.1.4`) — packed `major*256 + minor`.
const E3_IPM_INFO_FW_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 21695, 1, 10, 7, 1, 4];

/// Vendor MIB module for Bachmann PDUs.
#[derive(Debug, Default, Clone, Copy)]
pub struct BachmannMib;

#[async_trait]
impl MibSupport for BachmannMib {
    fn name(&self) -> &'static str {
        "bachmann-pdu"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), PUBLIC)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Bachmann".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &E3_IPM_INFO_SERIAL).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_number(session, &E3_IPM_INFO_FW_VERSION)
                .await?
                .map(|fwrev| format!("{}.{}", fwrev / 256, fwrev % 256));
        }

        if let Some(version) = get_number(session, &E3_IPM_INFO_HW_VERSION).await? {
            device.add_firmware(Firmware {
                name: Some("Hardware version".to_owned()),
                description: Some("Pdu hardware revision".to_owned()),
                r#type: Some("hardware".to_owned()),
                version: Some(version.to_string()),
                manufacturer: Some("Bachmann".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BachmannMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_bachmann() {
        assert!(BachmannMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.21695.1.10".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!BachmannMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.21695.2".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn decodes_packed_firmware_and_hardware() {
        // 0x0103 = 259 → "1.3"; hardware revision 2.
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.21695.1.10.7.1.1 = STRING: \"BM2401234\"\n\
             .1.3.6.1.4.1.21695.1.10.7.1.3 = INTEGER: 2\n\
             .1.3.6.1.4.1.21695.1.10.7.1.4 = INTEGER: 259\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        BachmannMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Bachmann"));
        assert_eq!(device.info.serial.as_deref(), Some("BM2401234"));
        assert_eq!(device.info.firmware.as_deref(), Some("1.3"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("2"));
    }
}
