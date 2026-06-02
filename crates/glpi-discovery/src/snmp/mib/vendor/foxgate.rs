// SPDX-License-Identifier: GPL-2.0-only

//! FoxGate vendor MIB support (networking).
//!
//! Applies to FoxGate devices (`1.3.6.1.4.1.6339`). The model comes from
//! `ntpEntSoftwareName` (falling back to the first `sysDescr` line) and the
//! firmware from `sysSoftwareVersion`; the serial and the BootRom/HardWare
//! versions are parsed out of the multi-line system description (the
//! `sysDescr` already captured into [`DeviceInfo::description`]). Ported from
//! the upstream `GLPI::Agent::SNMP::MibSupport::FoxGate`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// FoxGate enterprise OID.
const FOXGATE: &str = "1.3.6.1.4.1.6339";
/// `sysSoftwareVersion` (`os.1.3.0`).
const SYS_SOFTWARE_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 6339, 100, 1, 3, 0];
/// `ntpEntSoftwareName` (`os.25.1.1.1.0`).
const NTP_ENT_SOFTWARE_NAME: [u64; 13] = [1, 3, 6, 1, 4, 1, 6339, 100, 25, 1, 1, 1, 0];

/// Vendor MIB module for FoxGate devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct FoxGateMib;

#[async_trait]
impl MibSupport for FoxGateMib {
    fn name(&self) -> &'static str {
        "foxgate"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), FOXGATE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let sys_descr = device.info.description.clone().unwrap_or_default();

        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("FoxGate".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &SYS_SOFTWARE_VERSION).await?;
        }

        let model = match get_string(session, &NTP_ENT_SOFTWARE_NAME).await? {
            Some(name) => Some(name),
            None => model_from_descr(&sys_descr),
        };
        if device.info.serial.is_none() {
            device.info.serial = field_after(&sys_descr, "Device serial number");
        }
        if device.info.model.is_none() {
            device.info.model.clone_from(&model);
        }

        let label = model.as_deref().unwrap_or("FoxGate");
        if let Some(version) = field_after(&sys_descr, "BootRom Version") {
            device.add_firmware(make_firmware(
                &format!("{label} bootrom"),
                "bootrom version",
                version,
            ));
        }
        if let Some(version) = field_after(&sys_descr, "HardWare Version") {
            device.add_firmware(make_firmware(
                &format!("{label} hardware"),
                "hardware version",
                version,
            ));
        }
        Ok(())
    }
}

/// Builds a FoxGate "device"-type firmware entry.
fn make_firmware(name: &str, description: &str, version: String) -> Firmware {
    Firmware {
        name: Some(name.to_owned()),
        description: Some(description.to_owned()),
        r#type: Some("device".to_owned()),
        version: Some(version),
        manufacturer: Some("FoxGate".to_owned()),
    }
}

/// Model from the first `sysDescr` line of the form `<model> Device, …`.
fn model_from_descr(sys_descr: &str) -> Option<String> {
    let first = sys_descr.lines().next()?;
    let (model, _) = first.split_once(" Device,")?;
    (!model.is_empty()).then(|| model.to_owned())
}

/// Returns the single token following `label` on its `sysDescr` line.
fn field_after(sys_descr: &str, label: &str) -> Option<String> {
    sys_descr
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(label))
        .and_then(|line| line[label.len()..].split_whitespace().next())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::FoxGateMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_foxgate() {
        assert!(FoxGateMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.6339.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!FoxGateMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.633".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn parses_sysdescr_for_serial_and_versions() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.6339.100.1.3.0 = STRING: \"7.0.3.5\"\n").unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some(
                    "S6424 Device, Compatible with FoxGate\n\
                     Device serial number FG2401234567\n\
                     BootRom Version 1.1.3\n\
                     HardWare Version 2.0"
                        .to_owned(),
                ),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        FoxGateMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("FoxGate"));
        assert_eq!(device.info.firmware.as_deref(), Some("7.0.3.5"));
        assert_eq!(device.info.model.as_deref(), Some("S6424"));
        assert_eq!(device.info.serial.as_deref(), Some("FG2401234567"));
        assert_eq!(device.firmwares.len(), 2);
        assert_eq!(device.firmwares[0].name.as_deref(), Some("S6424 bootrom"));
        assert_eq!(device.firmwares[0].version.as_deref(), Some("1.1.3"));
        assert_eq!(device.firmwares[1].version.as_deref(), Some("2.0"));
    }
}
