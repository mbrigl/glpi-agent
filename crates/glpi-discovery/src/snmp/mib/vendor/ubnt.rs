// SPDX-License-Identifier: GPL-2.0-only

//! Ubiquiti (UniFi) vendor MIB support.
//!
//! Applies to Ubiquiti devices (`UBNT-MIB`, `1.3.6.1.4.1.41112`) and fills the
//! firmware, model and serial (the access-point MAC address without
//! separators) from the `UBNT-UniFi-MIB`. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Ubnt`; the per-radio SSID/port enrichment is
//! not modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;
use crate::snmp::value::SnmpValue;

/// Ubiquiti enterprise OID.
const UBNT: &str = "1.3.6.1.4.1.41112";
/// `ubntWlStatApMac` (`ubnt.1.4.5.1.4.1`) — the AP MAC, used as the serial.
const UBNT_WL_STAT_AP_MAC: [u64; 13] = [1, 3, 6, 1, 4, 1, 41112, 1, 4, 5, 1, 4, 1];
/// `unifiApSystemModel` (`ubnt.1.6.3.3.0`).
const UNIFI_AP_SYSTEM_MODEL: [u64; 12] = [1, 3, 6, 1, 4, 1, 41112, 1, 6, 3, 3, 0];
/// `unifiApSystemVersion` (`ubnt.1.6.3.6.0`).
const UNIFI_AP_SYSTEM_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 41112, 1, 6, 3, 6, 0];

/// Vendor MIB module for Ubiquiti devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct UbntMib;

#[async_trait]
impl MibSupport for UbntMib {
    fn name(&self) -> &'static str {
        "ubnt-unifi"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), UBNT)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &UNIFI_AP_SYSTEM_VERSION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &UNIFI_AP_SYSTEM_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = mac_serial(session.get(&UBNT_WL_STAT_AP_MAC).await?.as_ref());
        }
        Ok(())
    }
}

/// Derives a serial from the AP MAC: the 12 hex digits, with no separators.
fn mac_serial(value: Option<&SnmpValue>) -> Option<String> {
    match value {
        Some(SnmpValue::OctetString(bytes)) if bytes.len() == 6 => {
            Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
        }
        Some(other) => {
            let text = other.clone().as_str()?;
            let serial: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            (!serial.is_empty()).then_some(serial)
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::UbntMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_ubnt() {
        assert!(UbntMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.41112.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!UbntMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.41113".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_mac_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.41112.1.4.5.1.4.1 = Hex-STRING: 78 8A 20 11 22 33\n\
             .1.3.6.1.4.1.41112.1.6.3.3.0 = STRING: \"U7PG2\"\n\
             .1.3.6.1.4.1.41112.1.6.3.6.0 = STRING: \"4.3.28.11361\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        UbntMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.firmware.as_deref(), Some("4.3.28.11361"));
        assert_eq!(device.info.model.as_deref(), Some("U7PG2"));
        assert_eq!(device.info.serial.as_deref(), Some("788a20112233"));
    }
}
