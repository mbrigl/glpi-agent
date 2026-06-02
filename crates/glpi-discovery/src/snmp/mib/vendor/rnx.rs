// SPDX-License-Identifier: GPL-2.0-only

//! RNX PDU vendor MIB support.
//!
//! Applies to RNX power-distribution units (`RNX-UPDU-MIB2-MIB`,
//! `1.3.6.1.4.1.55108`). Fills the manufacturer, serial, firmware and model
//! (parsed from the system description). On GLPI 12+ the device is typed `PDU`
//! and its outlets are reported as `PDU.PLUGS` (number, name, connector type)
//! with the PDU part number; older servers get the `NETWORKING` type. Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::RNX`.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    as_number, get_string, pdu_type, sysobjectid_matches, table_index, DeviceInfo, MibSupport,
    NetworkDevice, Plug,
};
use crate::snmp::query::SnmpQuery;

/// RNX enterprise OID.
const RNX: &str = "1.3.6.1.4.1.55108";
/// `upduMib2PDUSerialNumber` (`upduMib2.1.2.1.5.1`).
const UPDU_PDU_SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 55108, 2, 1, 2, 1, 5, 1];
/// `upduMib2PDUPartNumber` (`upduMib2.1.2.1.6.1`).
const UPDU_PDU_PART_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 55108, 2, 1, 2, 1, 6, 1];
/// `upduMib2ICMFirmware` (`upduMib2.6.2.1.9.1`).
const UPDU_ICM_FIRMWARE: [u64; 13] = [1, 3, 6, 1, 4, 1, 55108, 2, 6, 2, 1, 9, 1];
/// `upduMib2OutletSystemName` (`upduMib2Outlet.2.1.2`) — per-outlet name column.
const UPDU_OUTLET_SYSTEM_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 55108, 2, 9, 2, 1, 2];
/// `upduMib2OutletCustomName` (`upduMib2Outlet.2.1.3`).
const UPDU_OUTLET_CUSTOM_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 55108, 2, 9, 2, 1, 3];
/// `upduMib2OutletRating` (`upduMib2Outlet.2.1.8`).
const UPDU_OUTLET_RATING: [u64; 12] = [1, 3, 6, 1, 4, 1, 55108, 2, 9, 2, 1, 8];

/// Vendor MIB module for RNX PDUs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RnxMib;

#[async_trait]
impl MibSupport for RnxMib {
    fn name(&self) -> &'static str {
        "rnx-pdu"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), RNX)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let device_type = pdu_type(device.glpi_version.as_deref());
        if device.info.r#type.is_none() {
            device.info.r#type = Some(device_type.to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("RNX".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &UPDU_PDU_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &UPDU_ICM_FIRMWARE).await?;
        }
        if device.info.model.is_none() {
            device.info.model = device
                .info
                .description
                .as_deref()
                .and_then(model_from_description);
        }

        // PDU outlets are only reported when the device is typed as a PDU
        // (GLPI 12+), matching the upstream guard.
        if device_type == "PDU" {
            let plugs = self.outlets(session).await?;
            if !plugs.is_empty() {
                device.pdu_mut().plugs = plugs;
            }
            if let Some(part) = get_string(session, &UPDU_PDU_PART_NUMBER).await? {
                device.pdu_mut().model = Some(part);
            }
        }
        Ok(())
    }
}

impl RnxMib {
    /// Walks the outlet table into `Plug`s (ordered by outlet number).
    async fn outlets(&self, session: &mut dyn SnmpQuery) -> Result<Vec<Plug>> {
        let system = walk_strings(session, &UPDU_OUTLET_SYSTEM_NAME).await?;
        let custom = walk_strings(session, &UPDU_OUTLET_CUSTOM_NAME).await?;
        let mut ratings: BTreeMap<u64, i64> = BTreeMap::new();
        for (oid, value) in session.walk(&UPDU_OUTLET_RATING).await? {
            if let (Some(index), Some(rating)) =
                (table_index(&oid, &UPDU_OUTLET_RATING), as_number(&value))
            {
                ratings.insert(index, rating);
            }
        }

        Ok(system
            .into_iter()
            .map(|(number, system_name)| {
                let name = custom
                    .get(&number)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .or(Some(system_name))
                    .filter(|n| !n.is_empty());
                Plug {
                    number: u32::try_from(number).unwrap_or(0),
                    name,
                    r#type: connector_type(ratings.get(&number).copied()).to_owned(),
                }
            })
            .collect())
    }
}

/// Maps an outlet current rating (mA) to its connector type.
fn connector_type(rating: Option<i64>) -> &'static str {
    match rating {
        Some(10000) => "C13",
        Some(16000) => "C19",
        _ => "unknown",
    }
}

/// Walks a string column into an index→value map (only non-empty values).
async fn walk_strings(session: &mut dyn SnmpQuery, base: &[u64]) -> Result<BTreeMap<u64, String>> {
    let mut map = BTreeMap::new();
    for (oid, value) in session.walk(base).await? {
        if let (Some(index), Some(text)) = (
            table_index(&oid, base),
            value.as_str().filter(|s| !s.is_empty()),
        ) {
            map.insert(index, text);
        }
    }
    Ok(map)
}

/// Extracts the model from an `RNX <model> (…)` system description.
fn model_from_description(description: &str) -> Option<String> {
    let rest = description.strip_prefix("RNX ")?;
    let model = rest[..rest.rfind(" (")?].trim();
    (!model.is_empty()).then(|| model.to_owned())
}

#[cfg(test)]
mod tests {
    use super::RnxMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_rnx() {
        assert!(RnxMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.55108.2".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RnxMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.55109".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    fn rnx_session() -> WalkSession {
        WalkSession::parse(
            ".1.3.6.1.4.1.55108.2.1.2.1.5.1 = STRING: \"RNX240100042\"\n\
             .1.3.6.1.4.1.55108.2.1.2.1.6.1 = STRING: \"UPDU-3PH-32A\"\n\
             .1.3.6.1.4.1.55108.2.6.2.1.9.1 = STRING: \"2.7.0\"\n\
             .1.3.6.1.4.1.55108.2.9.2.1.2.1 = STRING: \"Outlet 1\"\n\
             .1.3.6.1.4.1.55108.2.9.2.1.2.2 = STRING: \"Outlet 2\"\n\
             .1.3.6.1.4.1.55108.2.9.2.1.3.1 = STRING: \"Router\"\n\
             .1.3.6.1.4.1.55108.2.9.2.1.8.1 = INTEGER: 10000\n\
             .1.3.6.1.4.1.55108.2.9.2.1.8.2 = INTEGER: 16000\n",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn older_glpi_is_networking_without_plugs() {
        let mut session = rnx_session();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("RNX UPDU-3PH-32A (Rev 1.0)".to_owned()),
                ..DeviceInfo::default()
            },
            glpi_version: Some("10.0.19".to_owned()),
            ..NetworkDevice::default()
        };
        RnxMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.model.as_deref(), Some("UPDU-3PH-32A"));
        // No PDU section on older servers.
        assert!(device.pdu.is_none());
    }

    #[tokio::test]
    async fn glpi12_reports_pdu_with_plugs() {
        let mut session = rnx_session();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("RNX UPDU-3PH-32A (Rev 1.0)".to_owned()),
                ..DeviceInfo::default()
            },
            glpi_version: Some("12.0.0".to_owned()),
            ..NetworkDevice::default()
        };
        RnxMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("PDU"));

        let pdu = device.pdu.as_ref().unwrap();
        assert_eq!(pdu.model.as_deref(), Some("UPDU-3PH-32A"));
        assert_eq!(pdu.plugs.len(), 2);
        // Outlet 1: custom name wins; 10000 mA -> C13.
        assert_eq!(pdu.plugs[0].number, 1);
        assert_eq!(pdu.plugs[0].name.as_deref(), Some("Router"));
        assert_eq!(pdu.plugs[0].r#type, "C13");
        // Outlet 2: no custom name -> system name; 16000 mA -> C19.
        assert_eq!(pdu.plugs[1].number, 2);
        assert_eq!(pdu.plugs[1].name.as_deref(), Some("Outlet 2"));
        assert_eq!(pdu.plugs[1].r#type, "C19");
    }
}
