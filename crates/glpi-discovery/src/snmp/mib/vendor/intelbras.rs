// SPDX-License-Identifier: GPL-2.0-only

//! Intelbras vendor MIB support (networking).
//!
//! Applies to Intelbras devices, which expose the Dahua `DAHUA-SNMP-MIB`
//! `systemInfo` group (`1.3.6.1.4.1.1004849.2.1`). Sets the `NETWORKING` type
//! and fills manufacturer, serial, firmware and model, and records the
//! hardware- and system-version firmware entries. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Intelbras`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `systemInfo` (`dahua.2.1`) — the OID subtree Intelbras devices answer under.
const SYSTEM_INFO: &str = "1.3.6.1.4.1.1004849.2.1";
/// `softwareRevision` (`systemInfo.1.1.0`).
const SOFTWARE_REVISION: [u64; 12] = [1, 3, 6, 1, 4, 1, 1004849, 2, 1, 1, 1, 0];
/// `hardwareRevision` (`systemInfo.1.2.0`).
const HARDWARE_REVISION: [u64; 12] = [1, 3, 6, 1, 4, 1, 1004849, 2, 1, 1, 2, 0];
/// `serialNumber` (`systemInfo.2.4.0`).
const SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 1004849, 2, 1, 2, 4, 0];
/// `systemVersion` (`systemInfo.2.5.0`).
const SYSTEM_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 1004849, 2, 1, 2, 5, 0];
/// `deviceType` (`systemInfo.2.6.0`).
const DEVICE_TYPE: [u64; 12] = [1, 3, 6, 1, 4, 1, 1004849, 2, 1, 2, 6, 0];

/// Vendor MIB module for Intelbras devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct IntelbrasMib;

#[async_trait]
impl MibSupport for IntelbrasMib {
    fn name(&self) -> &'static str {
        "intelbras"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), SYSTEM_INFO)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Intelbras".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &SOFTWARE_REVISION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &DEVICE_TYPE).await?;
        }

        if let Some(version) = get_string(session, &HARDWARE_REVISION).await? {
            device.add_firmware(Firmware {
                name: Some("Intelbras hardware".to_owned()),
                description: Some("Hardware version".to_owned()),
                r#type: Some("hardware".to_owned()),
                version: Some(version),
                manufacturer: Some("Intelbras".to_owned()),
            });
        }
        if let Some(version) = get_string(session, &SYSTEM_VERSION).await? {
            device.add_firmware(Firmware {
                name: Some("Intelbras system".to_owned()),
                description: Some("System version".to_owned()),
                r#type: Some("system".to_owned()),
                version: Some(version),
                manufacturer: Some("Intelbras".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::IntelbrasMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_dahua_system_info() {
        assert!(IntelbrasMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1004849.2.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!IntelbrasMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1004849.1".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_firmwares() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1004849.2.1.1.1.0 = STRING: \"V2.400\"\n\
             .1.3.6.1.4.1.1004849.2.1.1.2.0 = STRING: \"R1\"\n\
             .1.3.6.1.4.1.1004849.2.1.2.4.0 = STRING: \"INT0099887766\"\n\
             .1.3.6.1.4.1.1004849.2.1.2.5.0 = STRING: \"S2.1\"\n\
             .1.3.6.1.4.1.1004849.2.1.2.6.0 = STRING: \"SG 2404 MR\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        IntelbrasMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Intelbras"));
        assert_eq!(device.info.serial.as_deref(), Some("INT0099887766"));
        assert_eq!(device.info.firmware.as_deref(), Some("V2.400"));
        assert_eq!(device.info.model.as_deref(), Some("SG 2404 MR"));
        assert_eq!(device.firmwares.len(), 2);
        assert_eq!(device.firmwares[0].r#type.as_deref(), Some("hardware"));
        assert_eq!(device.firmwares[1].version.as_deref(), Some("S2.1"));
    }
}
