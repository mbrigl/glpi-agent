// SPDX-License-Identifier: GPL-2.0-only

//! Telco Systems vendor MIB support (networking).
//!
//! Applies to Telco Systems switches (`PRVT-SWITCH-MIB`,
//! `1.3.6.1.4.1.738.1.5`). Sets the `NETWORKING` type, manufacturer, model and
//! serial, derives the firmware from the system description, and records the
//! hardware revision as a firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Telco`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `switch` (`prvt_products.5`) — the OID subtree Telco switches answer under.
const SWITCH: &str = "1.3.6.1.4.1.738.1.5";
/// `sysSerialNumber` (`prvtSwitchMib.1.3.1.0`).
const SYS_SERIAL_NUMBER: [u64; 14] = [1, 3, 6, 1, 4, 1, 738, 1, 5, 100, 1, 3, 1, 0];
/// `sysSwitchModel` (`prvtSwitchMib.1.3.2.0`).
const SYS_SWITCH_MODEL: [u64; 14] = [1, 3, 6, 1, 4, 1, 738, 1, 5, 100, 1, 3, 2, 0];
/// `sysHwRevision` (`prvtSwitchMib.1.3.6.0`).
const SYS_HW_REVISION: [u64; 14] = [1, 3, 6, 1, 4, 1, 738, 1, 5, 100, 1, 3, 6, 0];

/// Vendor MIB module for Telco Systems devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct TelcoMib;

#[async_trait]
impl MibSupport for TelcoMib {
    fn name(&self) -> &'static str {
        "telco-switch"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), SWITCH)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Telco Systems".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &SYS_SWITCH_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SYS_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = device
                .info
                .description
                .as_deref()
                .and_then(firmware_from_description);
        }

        if let Some(version) = get_string(session, &SYS_HW_REVISION).await? {
            let prefix = device
                .info
                .model
                .as_deref()
                .map_or(String::new(), |m| format!("{m} "));
            device.add_firmware(Firmware {
                name: Some(format!("{prefix}hardware")),
                description: Some(format!("{prefix}hardware revision")),
                r#type: Some("device".to_owned()),
                version: Some(version),
                manufacturer: Some("Telco Systems".to_owned()),
            });
        }
        Ok(())
    }
}

/// Extracts the token following "software version " in the system description.
fn firmware_from_description(description: &str) -> Option<String> {
    let rest = description.split("software version ").nth(1)?;
    rest.split_whitespace().next().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::TelcoMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_telco_switch_subtree() {
        assert!(TelcoMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.738.1.5.42".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!TelcoMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.738.1.4".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_firmware_and_hardware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.738.1.5.100.1.3.1.0 = STRING: \"TS0099\"\n\
             .1.3.6.1.4.1.738.1.5.100.1.3.2.0 = STRING: \"T-Marc 3306\"\n\
             .1.3.6.1.4.1.738.1.5.100.1.3.6.0 = STRING: \"B\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("T-Marc software version 6.1.2 build 7".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        TelcoMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Telco Systems"));
        assert_eq!(device.info.model.as_deref(), Some("T-Marc 3306"));
        assert_eq!(device.info.serial.as_deref(), Some("TS0099"));
        assert_eq!(device.info.firmware.as_deref(), Some("6.1.2"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("T-Marc 3306 hardware")
        );
        assert_eq!(device.firmwares[0].version.as_deref(), Some("B"));
    }
}
