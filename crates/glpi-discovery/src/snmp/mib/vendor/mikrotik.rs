// SPDX-License-Identifier: GPL-2.0-only

//! Mikrotik vendor MIB support.
//!
//! Applies to devices under the Mikrotik enterprise (`1.3.6.1.4.1.14988`).
//! Fills the serial number and firmware version from `MIKROTIK-MIB`
//! (`mtxrSerialNumber`, `mtxrFirmwareVersion`) and derives the model from the
//! `RouterOS …` `sysDescr` text. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Mikrotik`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Mikrotik enterprise OID.
const MIKROTIK_ENTERPRISE: &str = "1.3.6.1.4.1.14988";
/// `MIKROTIK-MIB::mtxrSerialNumber.0`.
const MTXR_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 14988, 1, 1, 7, 3, 0];
/// `MIKROTIK-MIB::mtxrFirmwareVersion.0`.
const MTXR_FIRMWARE_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 14988, 1, 1, 7, 4, 0];

/// Vendor MIB module for Mikrotik devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct MikrotikMib;

#[async_trait]
impl MibSupport for MikrotikMib {
    fn name(&self) -> &'static str {
        "mikrotik"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), MIKROTIK_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &MTXR_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &MTXR_FIRMWARE_VERSION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = device
                .info
                .description
                .as_deref()
                .and_then(parse_routeros_model);
        }
        Ok(())
    }
}

/// Extracts the model from a `RouterOS <model>` `sysDescr` string.
fn parse_routeros_model(description: &str) -> Option<String> {
    description
        .strip_prefix("RouterOS")
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{parse_routeros_model, MikrotikMib};
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_mikrotik() {
        assert!(MikrotikMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.14988.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!MikrotikMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[test]
    fn parses_routeros_model() {
        assert_eq!(
            parse_routeros_model("RouterOS CCR1009-7G-1C-1S+").as_deref(),
            Some("CCR1009-7G-1C-1S+")
        );
        assert_eq!(parse_routeros_model("Linux 5.6.3"), None);
    }

    #[tokio::test]
    async fn fills_serial_firmware_and_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.14988.1.1.7.3.0 = STRING: \"9C2A0987ABCD\"\n\
             .1.3.6.1.4.1.14988.1.1.7.4.0 = STRING: \"6.49.7\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        device.info.description = Some("RouterOS CCR1009-7G-1C-1S+".to_owned());
        MikrotikMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("9C2A0987ABCD"));
        assert_eq!(device.info.firmware.as_deref(), Some("6.49.7"));
        assert_eq!(device.info.model.as_deref(), Some("CCR1009-7G-1C-1S+"));
    }
}
