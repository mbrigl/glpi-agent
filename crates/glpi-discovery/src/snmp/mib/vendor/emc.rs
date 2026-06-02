// SPDX-License-Identifier: GPL-2.0-only

//! EMC (Dell EMC) vendor MIB support (storage).
//!
//! Applies to EMC devices (`1.3.6.1.4.1.674`) that expose the experimental
//! `FCMGMT-MIB` `connUnitTable`. The connection-unit table identifies the
//! storage unit; this module sets the `NETWORKING` type and fills the serial
//! and product model from the first table row. The type is only set when the
//! `connUnit` table is present, so EMC-branded printers are left untouched.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::EMC`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    instance_suffix, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// EMC enterprise OID.
const EMC: &str = "1.3.6.1.4.1.674";
/// `connUnitId` (`connUnitEntry.1`) — the FCMGMT connection-unit index column.
const CONN_UNIT_ID_COL: [u64; 10] = [1, 3, 6, 1, 3, 94, 1, 6, 1, 1];
/// `connUnitProduct` (`connUnitEntry.7`).
const CONN_UNIT_PRODUCT: [u64; 10] = [1, 3, 6, 1, 3, 94, 1, 6, 1, 7];
/// `connUnitSn` (`connUnitEntry.8`).
const CONN_UNIT_SN: [u64; 10] = [1, 3, 6, 1, 3, 94, 1, 6, 1, 8];

/// Vendor MIB module for EMC storage devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmcMib;

#[async_trait]
impl MibSupport for EmcMib {
    fn name(&self) -> &'static str {
        "emc"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), EMC)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // The first connection-unit row identifies the storage device.
        let mut suffixes: Vec<Vec<u64>> = session
            .walk(&CONN_UNIT_ID_COL)
            .await?
            .into_iter()
            .filter_map(|(oid, _)| instance_suffix(&oid, &CONN_UNIT_ID_COL))
            .collect();
        suffixes.sort();
        let Some(key) = suffixes.into_iter().next() else {
            return Ok(());
        };

        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.serial.is_none() {
            let oid: Vec<u64> = CONN_UNIT_SN.iter().chain(&key).copied().collect();
            device.info.serial = session.get(&oid).await?.and_then(|v| v.as_str());
        }
        if device.info.model.is_none() {
            let oid: Vec<u64> = CONN_UNIT_PRODUCT.iter().chain(&key).copied().collect();
            device.info.model = session
                .get(&oid)
                .await?
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::EmcMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_emc() {
        assert!(EmcMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.674.11000".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!EmcMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.675".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_first_connunit_row() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.3.94.1.6.1.1.16.0.96.22.0.0.1 = Hex-STRING: 10 00 00 60\n\
             .1.3.6.1.3.94.1.6.1.7.16.0.96.22.0.0.1 = STRING: \"VNX5400\"\n\
             .1.3.6.1.3.94.1.6.1.8.16.0.96.22.0.0.1 = STRING: \"CKM00123400567\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        EmcMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.model.as_deref(), Some("VNX5400"));
        assert_eq!(device.info.serial.as_deref(), Some("CKM00123400567"));
    }

    #[tokio::test]
    async fn no_connunit_table_leaves_device_untouched() {
        let mut session =
            WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"EMC printer\"\n").unwrap();
        let mut device = NetworkDevice::default();
        EmcMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type, None);
        assert_eq!(device.info.model, None);
    }
}
