// SPDX-License-Identifier: GPL-2.0-only

//! Hikvision vendor MIB support.
//!
//! Applies to Hikvision devices under either of the two enterprise OIDs they
//! use (`1.3.6.1.4.1.39165` and `1.3.6.1.4.1.50001`). Sets the manufacturer and
//! `NETWORKING` type, fills the model, and derives the serial from the entity
//! index when present, otherwise from the MAC address (dashes stripped). Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::Hikvision`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Primary Hikvision enterprise OID.
const HIKVISION_ENTERPRISE: &str = "1.3.6.1.4.1.39165";
/// Secondary Hikvision enterprise OID (newer firmware).
const HIKVISION_ENTERPRISE_2: &str = "1.3.6.1.4.1.50001";
/// `hikvisionModel.0`.
const HIKVISION_MODEL: [u64; 10] = [1, 3, 6, 1, 4, 1, 39165, 1, 1, 0];
/// `hikvisionMac.0`.
const HIKVISION_MAC: [u64; 10] = [1, 3, 6, 1, 4, 1, 39165, 1, 4, 0];
/// `hikEntityIndex.0` (under the `50001` enterprise).
const HIK_ENTITY_INDEX: [u64; 10] = [1, 3, 6, 1, 4, 1, 50001, 1, 3, 0];

/// Vendor MIB module for Hikvision devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct HikvisionMib;

#[async_trait]
impl MibSupport for HikvisionMib {
    fn name(&self) -> &'static str {
        "hikvision"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, HIKVISION_ENTERPRISE)
            || sysobjectid_matches(oid, HIKVISION_ENTERPRISE_2)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Hikvision".to_owned());
        }
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &HIKVISION_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = match get_string(session, &HIK_ENTITY_INDEX).await? {
                Some(index) => Some(index),
                None => get_string(session, &HIKVISION_MAC)
                    .await?
                    .map(|mac| mac.replace('-', "")),
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::HikvisionMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_both_enterprises() {
        assert!(HikvisionMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.39165.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(HikvisionMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.50001.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!HikvisionMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn prefers_entity_index_for_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.39165.1.1.0 = STRING: \"DS-2CD2042WD\"\n\
             .1.3.6.1.4.1.50001.1.3.0 = STRING: \"DS2CD2042WD20200101AAWR123\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HikvisionMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Hikvision"));
        assert_eq!(device.info.model.as_deref(), Some("DS-2CD2042WD"));
        assert_eq!(
            device.info.serial.as_deref(),
            Some("DS2CD2042WD20200101AAWR123")
        );
    }

    #[tokio::test]
    async fn falls_back_to_mac_for_serial() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.39165.1.4.0 = STRING: \"c0-56-e3-11-22-33\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        HikvisionMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("c056e3112233"));
    }
}
