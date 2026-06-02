// SPDX-License-Identifier: GPL-2.0-only

//! HP HTTP-management switch vendor MIB support.
//!
//! Applies to HP EtherSwitch devices with HTTP management
//! (`HP-ICF-OID hpEtherSwitch`, `1.3.6.1.4.1.11.2.3.7.11`). Fills the firmware
//! (ROM version) and serial, and records the HP Web Management software version
//! as a system firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::HPHttpManagement`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `hpEtherSwitch` — the matched sysObjectID prefix.
const HP_ETHER_SWITCH: &str = "1.3.6.1.4.1.11.2.3.7.11";
/// `hpHttpMgVersion` (`hpHttpMgNetCitizen.6.0`).
const HP_HTTP_MG_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 6];
/// `hpHttpMgROMVersion` (`hpHttpMgNetCitizen.8.0`).
const HP_HTTP_MG_ROM_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 8];
/// `hpHttpMgSerialNumber` (`hpHttpMgNetCitizen.9.0`).
const HP_HTTP_MG_SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 9];

/// Vendor MIB module for HP HTTP-managed switches.
#[derive(Debug, Default, Clone, Copy)]
pub struct HpHttpManagementMib;

#[async_trait]
impl MibSupport for HpHttpManagementMib {
    fn name(&self) -> &'static str {
        "hp-etherswitch"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), HP_ETHER_SWITCH)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.firmware.is_none() {
            device.info.firmware = scalar(session, &HP_HTTP_MG_ROM_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = scalar(session, &HP_HTTP_MG_SERIAL_NUMBER).await?;
        }

        if let Some(version) = scalar(session, &HP_HTTP_MG_VERSION).await? {
            device.add_firmware(Firmware {
                name: Some("HP-HttpMg-Version".to_owned()),
                description: Some("HP Web Management Software version".to_owned()),
                r#type: Some("system".to_owned()),
                version: Some(version),
                manufacturer: Some("HP".to_owned()),
            });
        }
        Ok(())
    }
}

/// Reads the `.0` instance of `oid` as a non-empty string.
async fn scalar(session: &mut dyn SnmpQuery, oid: &[u64]) -> Result<Option<String>> {
    let full: Vec<u64> = oid.iter().copied().chain(std::iter::once(0)).collect();
    get_string(session, &full).await
}

#[cfg(test)]
mod tests {
    use super::HpHttpManagementMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_hp_etherswitch() {
        assert!(HpHttpManagementMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.11.2.3.7.11.45".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!HpHttpManagementMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.11.2.3.7.12".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_firmware_serial_and_web_mgmt_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.11.2.36.1.1.2.6.0 = STRING: \"H.07.31\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.8.0 = STRING: \"H.06.01\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.9.0 = STRING: \"SG123ABC45\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HpHttpManagementMib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.firmware.as_deref(), Some("H.06.01"));
        assert_eq!(device.info.serial.as_deref(), Some("SG123ABC45"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("H.07.31"));
    }
}
