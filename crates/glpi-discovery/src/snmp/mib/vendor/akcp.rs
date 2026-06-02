// SPDX-License-Identifier: GPL-2.0-only

//! AKCP environmental-monitor vendor MIB support (networking).
//!
//! Applies to AKCP devices (`1.3.6.1.4.1.3854`) and sets the `NETWORKING` type,
//! manufacturer, hostname, the serial (the sensor-probe MAC with dash
//! separators) and the model/firmware parsed from the system description.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Akcp`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// AKCP enterprise OID.
const AKCP: &str = "1.3.6.1.4.1.3854";
/// `sensorProbeMAC` (`akcp.1.2.2.1.3.0`).
const SENSOR_PROBE_MAC: [u64; 13] = [1, 3, 6, 1, 4, 1, 3854, 1, 2, 2, 1, 3, 0];
/// `cfgSystemDescription` (`config.8.0`).
const CFG_SYSTEM_DESCRIPTION: [u64; 12] = [1, 3, 6, 1, 4, 1, 3854, 3, 2, 1, 8, 0];
/// `cfgSystemName` (`config.9.0`).
const CFG_SYSTEM_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 3854, 3, 2, 1, 9, 0];

/// Vendor MIB module for AKCP devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct AkcpMib;

#[async_trait]
impl MibSupport for AkcpMib {
    fn name(&self) -> &'static str {
        "akcp"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), AKCP)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("AKCP".to_owned());
        }
        if device.info.name.is_none() {
            device.info.name = get_string(session, &CFG_SYSTEM_NAME).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SENSOR_PROBE_MAC)
                .await?
                .and_then(|mac| mac_dashes(&mac));
        }

        let description = get_string(session, &CFG_SYSTEM_DESCRIPTION).await?;
        if let Some(description) = description {
            let tokens: Vec<&str> = description.split_whitespace().collect();
            if device.info.model.is_none() {
                if let (Some(a), Some(b)) = (tokens.first(), tokens.get(1)) {
                    device.info.model = Some(format!("{a} {b}"));
                }
            }
            if device.info.firmware.is_none() {
                device.info.firmware = tokens
                    .get(2)
                    .filter(|v| v.starts_with(|c: char| c.is_ascii_digit() && c != '0'))
                    .map(|v| (*v).to_owned());
            }
        }
        Ok(())
    }
}

/// Normalises a MAC string to dash-separated lowercase hex pairs.
fn mac_dashes(mac: &str) -> Option<String> {
    let hex: String = mac.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() != 12 {
        return None;
    }
    let pairs: Vec<String> = (0..12)
        .step_by(2)
        .map(|i| hex[i..i + 2].to_lowercase())
        .collect();
    Some(pairs.join("-"))
}

#[cfg(test)]
mod tests {
    use super::AkcpMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_akcp() {
        assert!(AkcpMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.3854.3".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!AkcpMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.3855".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_from_description_and_mac() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.3854.1.2.2.1.3.0 = STRING: \"00:0B:DC:11:22:33\"\n\
             .1.3.6.1.4.1.3854.3.2.1.8.0 = STRING: \"securityProbe 5ESV 2.1.0 build123\"\n\
             .1.3.6.1.4.1.3854.3.2.1.9.0 = STRING: \"serverroom-probe\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        AkcpMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("AKCP"));
        assert_eq!(device.info.name.as_deref(), Some("serverroom-probe"));
        assert_eq!(device.info.serial.as_deref(), Some("00-0b-dc-11-22-33"));
        assert_eq!(device.info.model.as_deref(), Some("securityProbe 5ESV"));
        assert_eq!(device.info.firmware.as_deref(), Some("2.1.0"));
    }
}
