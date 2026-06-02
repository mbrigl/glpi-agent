// SPDX-License-Identifier: GPL-2.0-only

//! WatchGuard vendor MIB support (networking / firewall).
//!
//! Applies to WatchGuard devices (`1.3.6.1.4.1.3097`). The software-version
//! scalar packs several versions as `<tag:value>` markers: `sysa` becomes the
//! device firmware and `sysb` a system firmware entry, while the Gateway
//! Antivirus and Intrusion Prevention service versions are recorded as service
//! firmwares. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::WatchGuard`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// WatchGuard enterprise OID.
const WATCHGUARD: &str = "1.3.6.1.4.1.3097";
/// `wgInfoGavService` (`wgInfoModule.1.3.0`).
const WG_INFO_GAV_SERVICE: [u64; 11] = [1, 3, 6, 1, 4, 1, 3097, 6, 1, 3, 0];
/// `wgInfoIpsService` (`wgInfoModule.1.4.0`).
const WG_INFO_IPS_SERVICE: [u64; 11] = [1, 3, 6, 1, 4, 1, 3097, 6, 1, 4, 0];
/// `wgSoftwareVersion` (`wgInfoModule.3.1.0`).
const WG_SOFTWARE_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 3097, 6, 3, 1, 0];

/// Vendor MIB module for WatchGuard devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct WatchGuardMib;

#[async_trait]
impl MibSupport for WatchGuardMib {
    fn name(&self) -> &'static str {
        "watchguard"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), WATCHGUARD)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("WatchGuard".to_owned());
        }

        let software = get_string(session, &WG_SOFTWARE_VERSION).await?;
        if device.info.firmware.is_none() {
            device.info.firmware = software.as_deref().and_then(|s| extract_tag(s, "sysa"));
        }

        let name = device
            .info
            .model
            .clone()
            .unwrap_or_else(|| "WatchGuard".to_owned());

        if let Some(version) = software.as_deref().and_then(|s| extract_tag(s, "sysb")) {
            device.add_firmware(service_firmware(
                &format!("{name} sysB"),
                &format!("{name} sysB software version"),
                "system",
                version,
            ));
        }
        if let Some(version) = get_string(session, &WG_INFO_GAV_SERVICE)
            .await?
            .as_deref()
            .and_then(|s| extract_tag(s, "gav_version"))
        {
            device.add_firmware(service_firmware(
                &format!("{name} GAV"),
                &format!("{name} Gateway Antivirus Service version"),
                "service",
                version,
            ));
        }
        if let Some(version) = get_string(session, &WG_INFO_IPS_SERVICE)
            .await?
            .as_deref()
            .and_then(|s| extract_tag(s, "ips_version"))
        {
            device.add_firmware(service_firmware(
                &format!("{name} IPS"),
                &format!("{name} Intrusion Prevention Service version"),
                "service",
                version,
            ));
        }
        Ok(())
    }
}

/// Builds a WatchGuard firmware entry of the given type.
fn service_firmware(name: &str, description: &str, kind: &str, version: String) -> Firmware {
    Firmware {
        name: Some(name.to_owned()),
        description: Some(description.to_owned()),
        r#type: Some(kind.to_owned()),
        version: Some(version),
        manufacturer: Some("WatchGuard".to_owned()),
    }
}

/// Extracts the value of a `<tag:value>` marker from `s`.
fn extract_tag(s: &str, tag: &str) -> Option<String> {
    let marker = format!("<{tag}:");
    let start = s.find(&marker)? + marker.len();
    let end = s[start..].find('>')? + start;
    let value = &s[start..end];
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::WatchGuardMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_watchguard() {
        assert!(WatchGuardMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.3097.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!WatchGuardMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.3098".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn extracts_tagged_versions() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.3097.6.3.1.0 = STRING: \"<sysa:12.5.2><sysb:12.5.1>\"\n\
             .1.3.6.1.4.1.3097.6.1.3.0 = STRING: \"<gav_version:1.456>\"\n\
             .1.3.6.1.4.1.3097.6.1.4.0 = STRING: \"<ips_version:4.789>\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                model: Some("Firebox T40".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        WatchGuardMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("WatchGuard"));
        assert_eq!(device.info.firmware.as_deref(), Some("12.5.2"));
        assert_eq!(device.firmwares.len(), 3);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("Firebox T40 sysB")
        );
        assert_eq!(device.firmwares[0].version.as_deref(), Some("12.5.1"));
        assert_eq!(device.firmwares[1].name.as_deref(), Some("Firebox T40 GAV"));
        assert_eq!(device.firmwares[1].version.as_deref(), Some("1.456"));
        assert_eq!(device.firmwares[2].version.as_deref(), Some("4.789"));
    }
}
