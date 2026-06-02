// SPDX-License-Identifier: GPL-2.0-only

//! Htek IP phone vendor MIB support.
//!
//! Applies to Htek phones (`UNICORN-MIB`, `1.3.6.1.4.1.38241`) and fills the
//! model and the firmware (the version following the `BOOT--` prefix of the
//! firmware string). Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Htek`; the device-level MAC/IP fix-ups are
//! not modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Htek enterprise OID.
const HTEK: &str = "1.3.6.1.4.1.38241";
/// `firmware` (`htek.1.1.0`).
const FIRMWARE: [u64; 10] = [1, 3, 6, 1, 4, 1, 38241, 1, 1, 0];
/// `model` (`htek.1.2.0`).
const MODEL: [u64; 10] = [1, 3, 6, 1, 4, 1, 38241, 1, 2, 0];

/// Vendor MIB module for Htek IP phones.
#[derive(Debug, Default, Clone, Copy)]
pub struct HtekMib;

#[async_trait]
impl MibSupport for HtekMib {
    fn name(&self) -> &'static str {
        "htek"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), HTEK)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &MODEL).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &FIRMWARE)
                .await?
                .and_then(|fw| firmware_version(&fw));
        }
        Ok(())
    }
}

/// Extracts the version after a leading `BOOT--`.
fn firmware_version(firmware: &str) -> Option<String> {
    let rest = firmware.strip_prefix("BOOT--")?;
    let version: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!version.is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use super::HtekMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_htek() {
        assert!(HtekMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.38241.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!HtekMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.38242".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn parses_boot_firmware_and_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.38241.1.1.0 = STRING: \"BOOT--1.0.4.6 main\"\n\
             .1.3.6.1.4.1.38241.1.2.0 = STRING: \"UC924\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HtekMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("UC924"));
        assert_eq!(device.info.firmware.as_deref(), Some("1.0.4.6"));
    }
}
