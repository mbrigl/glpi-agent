// SPDX-License-Identifier: GPL-2.0-only

//! Hitachi Vantara vendor MIB support (storage).
//!
//! Applies to Hitachi Vantara arrays whose `sysObjectID` falls under
//! `hitachiVslSysObjectID` (`1.3.6.1.4.1.116.3.11.4.1.1`). The identity lives in
//! the first row of `raidExMibRaidListEntry`: serial (column 1), firmware
//! (column 3) and model (column 4). Devices whose model starts with `VSP` are
//! classified as `STORAGE`, the rest as `NETWORKING`. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::HitachiVantara`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    instance_suffix, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `hitachiVslSysObjectID` — the sysObjectID prefix of Hitachi arrays.
const HITACHI_VSL_SYSOBJECTID: &str = "1.3.6.1.4.1.116.3.11.4.1.1";
/// `raidExMibRaidListEntry` (`hitachi.5.11.4.1.1.5.1`).
const RAID_LIST_ENTRY: [u64; 14] = [1, 3, 6, 1, 4, 1, 116, 5, 11, 4, 1, 1, 5, 1];
/// `raidlistSerialNumber` (`raidExMibRaidListEntry.1`) — the column walked for
/// the row key (its value is also the serial).
const RAID_LIST_SERIAL: [u64; 15] = [1, 3, 6, 1, 4, 1, 116, 5, 11, 4, 1, 1, 5, 1, 1];

/// Vendor MIB module for Hitachi Vantara storage devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct HitachiVantaraMib;

#[async_trait]
impl MibSupport for HitachiVantaraMib {
    fn name(&self) -> &'static str {
        "hitachi-vantara"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), HITACHI_VSL_SYSOBJECTID)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // Resolve the (first) device row key from the serial-number column.
        let mut keys: Vec<Vec<u64>> = session
            .walk(&RAID_LIST_SERIAL)
            .await?
            .into_iter()
            .filter_map(|(oid, _)| instance_suffix(&oid, &RAID_LIST_SERIAL))
            .collect();
        keys.sort();
        let Some(key) = keys.into_iter().next() else {
            return Ok(());
        };

        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Hitachi Vantara".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = self.column(session, 1, &key).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = self.column(session, 3, &key).await?;
        }
        let model = self.column(session, 4, &key).await?;
        if device.info.r#type.is_none() {
            let is_vsp = model.as_deref().is_some_and(|m| m.starts_with("VSP"));
            device.info.r#type = Some(if is_vsp { "STORAGE" } else { "NETWORKING" }.to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = model;
        }
        Ok(())
    }
}

impl HitachiVantaraMib {
    /// Reads `raidExMibRaidListEntry.<column>.<key>` as a non-empty string.
    async fn column(
        &self,
        session: &mut dyn SnmpQuery,
        column: u64,
        key: &[u64],
    ) -> Result<Option<String>> {
        let oid: Vec<u64> = RAID_LIST_ENTRY
            .iter()
            .copied()
            .chain(std::iter::once(column))
            .chain(key.iter().copied())
            .collect();
        Ok(session
            .get(&oid)
            .await?
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::HitachiVantaraMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_hitachi_vsl() {
        assert!(HitachiVantaraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.116.3.11.4.1.1.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!HitachiVantaraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.116.5.11".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_vsp_row_as_storage() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.116.5.11.4.1.1.5.1.1.0 = STRING: \"410123\"\n\
             .1.3.6.1.4.1.116.5.11.4.1.1.5.1.3.0 = STRING: \"90-08-42/00\"\n\
             .1.3.6.1.4.1.116.5.11.4.1.1.5.1.4.0 = STRING: \"VSP E790\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HitachiVantaraMib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Hitachi Vantara"));
        assert_eq!(device.info.serial.as_deref(), Some("410123"));
        assert_eq!(device.info.firmware.as_deref(), Some("90-08-42/00"));
        assert_eq!(device.info.model.as_deref(), Some("VSP E790"));
        assert_eq!(device.info.r#type.as_deref(), Some("STORAGE"));
    }

    #[tokio::test]
    async fn non_vsp_model_is_networking() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.116.5.11.4.1.1.5.1.1.0 = STRING: \"sn\"\n\
             .1.3.6.1.4.1.116.5.11.4.1.1.5.1.4.0 = STRING: \"HUS 130\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HitachiVantaraMib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.model.as_deref(), Some("HUS 130"));
    }
}
