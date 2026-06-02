// SPDX-License-Identifier: GPL-2.0-only

//! D-Link DGS-1210 series vendor MIB support (networking).
//!
//! Applies to the D-Link DGS-1210 `companySystem` group
//! (`1.3.6.1.4.1.171.11.153.1000.1`). Sets the `NETWORKING` type, manufacturer,
//! firmware, serial and hostname, and records the hardware revision as a
//! firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::DlinkDGS1210Series`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `companySystem` (`dlink_dgs_1210_Common.1`) — the OID subtree these switches
/// answer under.
const COMPANY_SYSTEM: &str = "1.3.6.1.4.1.171.11.153.1000.1";
/// `sysSwitchName` (`companySystem.1.0`).
const SYS_SWITCH_NAME: [u64; 13] = [1, 3, 6, 1, 4, 1, 171, 11, 153, 1000, 1, 1, 0];
/// `sysHardwareVersion` (`companySystem.2.0`).
const SYS_HARDWARE_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 171, 11, 153, 1000, 1, 2, 0];
/// `sysFirmwareVersion` (`companySystem.3.0`).
const SYS_FIRMWARE_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 171, 11, 153, 1000, 1, 3, 0];
/// `sysSerialNumber` (`companySystem.33.1.0`).
const SYS_SERIAL_NUMBER: [u64; 14] = [1, 3, 6, 1, 4, 1, 171, 11, 153, 1000, 1, 33, 1, 0];

/// Vendor MIB module for D-Link DGS-1210 series switches.
#[derive(Debug, Default, Clone, Copy)]
pub struct DlinkDgs1210Mib;

#[async_trait]
impl MibSupport for DlinkDgs1210Mib {
    fn name(&self) -> &'static str {
        "d-link-dgs1210-series"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), COMPANY_SYSTEM)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("D-Link".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &SYS_FIRMWARE_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SYS_SERIAL_NUMBER).await?;
        }
        if device.info.name.is_none() {
            device.info.name = get_string(session, &SYS_SWITCH_NAME).await?;
        }

        if let Some(version) = get_string(session, &SYS_HARDWARE_VERSION).await? {
            let prefix = device
                .info
                .model
                .as_deref()
                .map_or(String::new(), |m| format!("{m} "));
            device.add_firmware(Firmware {
                name: Some(format!("{prefix}hardware")),
                description: Some("hardware revision".to_owned()),
                r#type: Some("device".to_owned()),
                version: Some(version),
                manufacturer: Some("D-Link".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DlinkDgs1210Mib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_dgs1210_company_system() {
        assert!(DlinkDgs1210Mib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.171.11.153.1000.1.5".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!DlinkDgs1210Mib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.171.10.153".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_hardware_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.171.11.153.1000.1.1.0 = STRING: \"DGS-1210-28\"\n\
             .1.3.6.1.4.1.171.11.153.1000.1.2.0 = STRING: \"C1\"\n\
             .1.3.6.1.4.1.171.11.153.1000.1.3.0 = STRING: \"6.30.016\"\n\
             .1.3.6.1.4.1.171.11.153.1000.1.33.1.0 = STRING: \"DGS1210SN001\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        DlinkDgs1210Mib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("D-Link"));
        assert_eq!(device.info.firmware.as_deref(), Some("6.30.016"));
        assert_eq!(device.info.serial.as_deref(), Some("DGS1210SN001"));
        assert_eq!(device.info.name.as_deref(), Some("DGS-1210-28"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("C1"));
    }
}
