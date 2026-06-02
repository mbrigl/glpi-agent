// SPDX-License-Identifier: GPL-2.0-only

//! MultiTech vendor MIB support (networking).
//!
//! MultiTech modules expose a private router-system group
//! (`1.3.6.1.4.1.995.15.1.1`) and often provide no standard `sysObjectID`.
//! Upstream selects this module by the model-id OID's presence; we gate on the
//! MultiTech enterprise prefix and self-guard in `run`. Sets the `NETWORKING`
//! type, manufacturer, serial, model, firmware and a `model_serial` hostname.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Multitech`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// MultiTech enterprise OID.
const MULTITECH: &str = "1.3.6.1.4.1.995";
/// `mtsRouterSystemModelId` (`mtsRouterSystemObjects.1.0`).
const MODEL_ID: [u64; 12] = [1, 3, 6, 1, 4, 1, 995, 15, 1, 1, 1, 0];
/// `mtsRouterSystemSerialNumber` (`mtsRouterSystemObjects.2.0`).
const SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 995, 15, 1, 1, 2, 0];
/// `mtsRouterSystemFirmware` (`mtsRouterSystemObjects.3.0`).
const FIRMWARE: [u64; 12] = [1, 3, 6, 1, 4, 1, 995, 15, 1, 1, 3, 0];

/// Vendor MIB module for MultiTech devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct MultitechMib;

#[async_trait]
impl MibSupport for MultitechMib {
    fn name(&self) -> &'static str {
        "multitech"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), MULTITECH)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let Some(model) = get_string(session, &MODEL_ID).await? else {
            return Ok(());
        };
        let serial = get_string(session, &SERIAL_NUMBER).await?;

        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Multitech".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &FIRMWARE).await?;
        }
        if device.info.name.is_none() {
            if let Some(serial) = serial.as_deref() {
                device.info.name = Some(format!("{model}_{serial}"));
            }
        }
        if device.info.serial.is_none() {
            device.info.serial = serial;
        }
        if device.info.model.is_none() {
            device.info.model = Some(model);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MultitechMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_multitech() {
        assert!(MultitechMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.995.15".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!MultitechMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.996".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_computes_hostname() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.995.15.1.1.1.0 = STRING: \"MTR-LVW2\"\n\
             .1.3.6.1.4.1.995.15.1.1.2.0 = STRING: \"20890123\"\n\
             .1.3.6.1.4.1.995.15.1.1.3.0 = STRING: \"6.0.2\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        MultitechMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Multitech"));
        assert_eq!(device.info.model.as_deref(), Some("MTR-LVW2"));
        assert_eq!(device.info.serial.as_deref(), Some("20890123"));
        assert_eq!(device.info.firmware.as_deref(), Some("6.0.2"));
        assert_eq!(device.info.name.as_deref(), Some("MTR-LVW2_20890123"));
    }
}
