// SPDX-License-Identifier: GPL-2.0-only

//! Snom IP phone vendor MIB support.
//!
//! Snom phones expose a single firmware scalar (`1.3.6.1.2.1.7526.2.4`) holding
//! `"<model> <version> <uboot>"`. Upstream selects this module by that private
//! OID's presence; lacking session access in `applies_to`, we gate on the Snom
//! OID prefix and self-guard in `run` (returning early when the scalar is
//! absent). Sets the `NETWORKING` type, manufacturer, model and firmware, and
//! records the U-Boot version as a firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Snom`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `snom` base OID.
const SNOM: &str = "1.3.6.1.2.1.7526";
/// `firmware` (`snom.2.4`) — `"<model> <version> <uboot>"`.
const FIRMWARE: [u64; 8] = [1, 3, 6, 1, 2, 1, 7526, 2];

/// Vendor MIB module for Snom phones.
#[derive(Debug, Default, Clone, Copy)]
pub struct SnomMib;

#[async_trait]
impl MibSupport for SnomMib {
    fn name(&self) -> &'static str {
        "snom"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), SNOM)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let oid: Vec<u64> = FIRMWARE.iter().copied().chain(std::iter::once(4)).collect();
        let Some(firmware) = get_string(session, &oid).await? else {
            return Ok(());
        };
        let mut parts = firmware.split_whitespace();
        let model = parts.next();
        let version = parts.next();
        let uboot = parts.next();

        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Snom".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = model.map(str::to_owned);
        }
        if device.info.firmware.is_none() {
            device.info.firmware = version.map(str::to_owned);
        }

        if let Some(uboot) = uboot {
            device.add_firmware(Firmware {
                name: Some("Snom Uboot version".to_owned()),
                description: Some("Snom Uboot firmware".to_owned()),
                r#type: Some("system".to_owned()),
                version: Some(uboot.to_owned()),
                manufacturer: Some("Snom".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SnomMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_snom_prefix() {
        assert!(SnomMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.2.1.7526.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!SnomMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn splits_firmware_scalar() {
        let mut session =
            WalkSession::parse(".1.3.6.1.2.1.7526.2.4 = STRING: \"snomD785 10.1.46.16 1.2\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        SnomMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Snom"));
        assert_eq!(device.info.model.as_deref(), Some("snomD785"));
        assert_eq!(device.info.firmware.as_deref(), Some("10.1.46.16"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("1.2"));
    }

    #[tokio::test]
    async fn no_firmware_scalar_leaves_device_untouched() {
        let mut session =
            WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"unrelated\"\n").unwrap();
        let mut device = NetworkDevice::default();
        SnomMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer, None);
    }
}
