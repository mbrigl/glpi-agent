// SPDX-License-Identifier: GPL-2.0-only

//! HW group (HWg) vendor MIB support (networking).
//!
//! Applies to HWg devices (`1.3.6.1.4.1.21796`) and sets the `NETWORKING` type,
//! manufacturer, the serial (the first available device MAC, without
//! separators) and the model (the system description). Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Hwg`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// HWg enterprise OID.
const HWG: &str = "1.3.6.1.4.1.21796";
/// `hwgWldMac` (`hwg.4.5.70.1.0`).
const HWG_WLD_MAC: [u64; 12] = [1, 3, 6, 1, 4, 1, 21796, 4, 5, 70, 1, 0];
/// `hwgSteMac` (`hwg.4.1.70.1.0`).
const HWG_STE_MAC: [u64; 12] = [1, 3, 6, 1, 4, 1, 21796, 4, 1, 70, 1, 0];
/// `hwgSte2Mac` (`hwg.4.9.70.1.0`).
const HWG_STE2_MAC: [u64; 12] = [1, 3, 6, 1, 4, 1, 21796, 4, 9, 70, 1, 0];

/// Vendor MIB module for HWg devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct HwgMib;

#[async_trait]
impl MibSupport for HwgMib {
    fn name(&self) -> &'static str {
        "hwg"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), HWG)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("HW group s.r.o".to_owned());
        }
        if device.info.serial.is_none() {
            for oid in [&HWG_WLD_MAC, &HWG_STE_MAC, &HWG_STE2_MAC] {
                if let Some(serial) = get_string(session, oid)
                    .await?
                    .and_then(|mac| mac_hex(&mac))
                {
                    device.info.serial = Some(serial);
                    break;
                }
            }
        }
        if device.info.model.is_none() {
            device.info.model.clone_from(&device.info.description);
        }
        Ok(())
    }
}

/// Returns the 12 lowercase hex digits of a MAC string, with no separators.
fn mac_hex(mac: &str) -> Option<String> {
    let hex: String = mac.chars().filter(char::is_ascii_hexdigit).collect();
    (hex.len() == 12).then(|| hex.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::HwgMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_hwg() {
        assert!(HwgMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.21796.4".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!HwgMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.21797".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_serial_from_mac_and_model_from_description() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.21796.4.1.70.1.0 = STRING: \"00:0A:59:AA:BB:CC\"\n")
                .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("HWg-STE2".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        HwgMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("HW group s.r.o"));
        assert_eq!(device.info.serial.as_deref(), Some("000a59aabbcc"));
        assert_eq!(device.info.model.as_deref(), Some("HWg-STE2"));
    }
}
