// SPDX-License-Identifier: GPL-2.0-only

//! Zebra printer vendor MIB support.
//!
//! Applies to Zebra printers under the ESI-MIB (`1.3.6.1.4.1.683`) and the
//! ZEBRA-MIB general-info group (`1.3.6.1.4.1.10642.1.1`). Fills manufacturer,
//! serial, model, firmware and hostname from an ordered list of candidate OIDs
//! across the ESI, ZEBRA and ZEBRA-QL MIBs, and records the LinkOS version as a
//! firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Zebra`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// ESI-MIB root (`esi`).
const ESI: &str = "1.3.6.1.4.1.683";
/// `zbrGeneralModel` (`zebra.1.1`) — the second matched sysObjectID prefix.
const ZBR_GENERAL_MODEL: &str = "1.3.6.1.4.1.10642.1.1";

/// `esi.serial` (`683.1.5.0`).
const ESI_SERIAL: [u64; 10] = [1, 3, 6, 1, 4, 1, 683, 1, 5, 0];
/// `esi.fw2` (`683.1.9.0`).
const ESI_FW2: [u64; 10] = [1, 3, 6, 1, 4, 1, 683, 1, 9, 0];
/// `model2` (`683.6.2.3.2.1.15.1`).
const MODEL2: [u64; 14] = [1, 3, 6, 1, 4, 1, 683, 6, 2, 3, 2, 1, 15, 1];
/// `model1` (`zbrGeneralModel.0`, `10642.1.1.0`).
const MODEL1: [u64; 11] = [1, 3, 6, 1, 4, 1, 10642, 1, 1, 0, 0];
/// `zbrGeneralFirmwareVersion` (`10642.1.2.0`).
const ZBR_FIRMWARE_VERSION: [u64; 10] = [1, 3, 6, 1, 4, 1, 10642, 1, 2, 0];
/// `zbrGeneralName` (`10642.1.4.0`).
const ZBR_NAME: [u64; 10] = [1, 3, 6, 1, 4, 1, 10642, 1, 4, 0];
/// `zbrGeneralUniqueId` (`10642.1.9.0`).
const ZBR_UNIQUE_ID: [u64; 10] = [1, 3, 6, 1, 4, 1, 10642, 1, 9, 0];
/// `zbrGeneralCompanyName` (`10642.1.11.0`).
const ZBR_COMPANY_NAME: [u64; 10] = [1, 3, 6, 1, 4, 1, 10642, 1, 11, 0];
/// `zbrGeneralLINKOSVersion` (`10642.1.18.0`).
const ZBR_LINKOS_VERSION: [u64; 10] = [1, 3, 6, 1, 4, 1, 10642, 1, 18, 0];
/// `model3` (`zql_zebra_ql.19.7.0`, `10642.200.19.7.0`).
const MODEL3: [u64; 11] = [1, 3, 6, 1, 4, 1, 10642, 200, 19, 7, 0];
/// `serial3` (`zql_zebra_ql.19.5.0`, `10642.200.19.5.0`).
const SERIAL3: [u64; 11] = [1, 3, 6, 1, 4, 1, 10642, 200, 19, 5, 0];

/// Vendor MIB module for Zebra printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZebraMib;

#[async_trait]
impl MibSupport for ZebraMib {
    fn name(&self) -> &'static str {
        "zebra-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, ESI) || sysobjectid_matches(oid, ZBR_GENERAL_MODEL)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let manufacturer = get_string(session, &ZBR_COMPANY_NAME)
            .await?
            .unwrap_or_else(|| "Zebra Technologies".to_owned());
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(manufacturer.clone());
        }
        if device.info.name.is_none() {
            device.info.name = get_string(session, &ZBR_NAME).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial =
                first_of(session, &[&ZBR_UNIQUE_ID, &SERIAL3, &ESI_SERIAL]).await?;
        }
        if device.info.model.is_none() {
            device.info.model = first_of(session, &[&MODEL1, &MODEL2, &MODEL3]).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = first_of(session, &[&ZBR_FIRMWARE_VERSION, &ESI_FW2]).await?;
        }

        if let Some(version) = get_string(session, &ZBR_LINKOS_VERSION).await? {
            device.add_firmware(Firmware {
                name: Some(format!("{manufacturer} LinkOS")),
                description: Some(format!("{manufacturer} LinkOS firmware")),
                r#type: Some("system".to_owned()),
                version: Some(version),
                manufacturer: Some(manufacturer),
            });
        }
        Ok(())
    }
}

/// Returns the first non-empty string among `oids`, in order.
async fn first_of(session: &mut dyn SnmpQuery, oids: &[&[u64]]) -> Result<Option<String>> {
    for oid in oids {
        if let Some(value) = get_string(session, oid).await? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::ZebraMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_esi_and_zebra_general_model() {
        assert!(ZebraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.683.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(ZebraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.10642.1.1.0".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!ZebraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.10642.200".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_from_zebra_mib() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.10642.1.1.0.0 = STRING: \"ZT411\"\n\
             .1.3.6.1.4.1.10642.1.2.0 = STRING: \"V93.21.01Z\"\n\
             .1.3.6.1.4.1.10642.1.4.0 = STRING: \"warehouse-zt1\"\n\
             .1.3.6.1.4.1.10642.1.9.0 = STRING: \"D8J200100123\"\n\
             .1.3.6.1.4.1.10642.1.11.0 = STRING: \"Zebra Technologies\"\n\
             .1.3.6.1.4.1.10642.1.18.0 = STRING: \"6.3\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        ZebraMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(
            device.info.manufacturer.as_deref(),
            Some("Zebra Technologies")
        );
        assert_eq!(device.info.name.as_deref(), Some("warehouse-zt1"));
        assert_eq!(device.info.serial.as_deref(), Some("D8J200100123"));
        assert_eq!(device.info.model.as_deref(), Some("ZT411"));
        assert_eq!(device.info.firmware.as_deref(), Some("V93.21.01Z"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("Zebra Technologies LinkOS")
        );
    }
}
