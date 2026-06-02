// SPDX-License-Identifier: GPL-2.0-only

//! Raritan PDU vendor MIB support.
//!
//! Applies to Raritan PDU2 devices (`PDU2-MIB`, `1.3.6.1.4.1.13742.6`). Fills
//! the manufacturer, model, serial and hostname from the nameplate / unit
//! configuration. On GLPI 12+ the device is typed `PDU` and its outlets are
//! reported as `PDU.PLUGS` (number, name, receptacle descriptor) with the rated
//! current as `PDU.TYPE`; older servers get `NETWORKING`. Ported from the
//! upstream `GLPI::Agent::SNMP::MibSupport::Raritan`.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, pdu_type, sysobjectid_matches, table_index, DeviceInfo, MibSupport, NetworkDevice,
    Plug,
};
use crate::snmp::query::SnmpQuery;

/// Raritan `pdu2` OID (`raritan.6`).
const RARITAN_PDU2: &str = "1.3.6.1.4.1.13742.6";
/// `pduManufacturer` (`nameplateEntry.2.1`).
const PDU_MANUFACTURER: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 2, 1];
/// `pduModel` (`nameplateEntry.3.1`).
const PDU_MODEL: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 3, 1];
/// `pduSerialNumber` (`nameplateEntry.4.1`).
const PDU_SERIAL_NUMBER: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 4, 1];
/// `pduRatedCurrent` (`nameplateEntry.6.1`) — used as `PDU.TYPE`.
const PDU_RATED_CURRENT: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 6, 1];
/// `pduName` (`unitConfigurationEntry.13.1`) — the device hostname.
const PDU_NAME: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 2, 1, 13, 1];
/// `outletConfigurationEntry` (`outlet.3.1`) — the per-outlet table; the column
/// id is the next arc: 2 = label, 3 = name, 29 = receptacle descriptor.
const OUTLET_TABLE: [u64; 12] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 5, 3, 1];
const COL_LABEL: u64 = 2;
const COL_NAME: u64 = 3;
const COL_DESCRIPTOR: u64 = 29;

/// Vendor MIB module for Raritan PDUs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RaritanMib;

#[async_trait]
impl MibSupport for RaritanMib {
    fn name(&self) -> &'static str {
        "raritan-pdu2"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), RARITAN_PDU2)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let device_type = pdu_type(device.glpi_version.as_deref());
        if device.info.r#type.is_none() {
            device.info.r#type = Some(device_type.to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(
                get_string(session, &PDU_MANUFACTURER)
                    .await?
                    .unwrap_or_else(|| "Raritan".to_owned()),
            );
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PDU_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &PDU_SERIAL_NUMBER).await?;
        }
        if device.info.name.is_none() {
            device.info.name = get_string(session, &PDU_NAME).await?;
        }

        // Outlets are only reported when typed as a PDU (GLPI 12+).
        if device_type == "PDU" {
            let plugs = self.outlets(session).await?;
            if !plugs.is_empty() {
                device.pdu_mut().plugs = plugs;
            }
            if let Some(rated_current) = get_string(session, &PDU_RATED_CURRENT).await? {
                device.pdu_mut().model = Some(rated_current);
            }
        }
        Ok(())
    }
}

impl RaritanMib {
    /// Walks the outlet table (label / name / receptacle descriptor) into
    /// `Plug`s, ordered by outlet index.
    async fn outlets(&self, session: &mut dyn SnmpQuery) -> Result<Vec<Plug>> {
        let labels = walk_column(session, &OUTLET_TABLE, COL_LABEL).await?;
        let names = walk_column(session, &OUTLET_TABLE, COL_NAME).await?;
        let descriptors = walk_column(session, &OUTLET_TABLE, COL_DESCRIPTOR).await?;

        Ok(labels
            .into_iter()
            .map(|(index, label)| {
                let name = names
                    .get(&index)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .or_else(|| (!label.is_empty()).then(|| label.clone()));
                Plug {
                    // The outlet label is usually its number; fall back to the
                    // table index when it is not purely numeric.
                    number: label
                        .parse()
                        .unwrap_or_else(|_| u32::try_from(index).unwrap_or(0)),
                    name,
                    r#type: descriptors.get(&index).cloned().unwrap_or_default(),
                }
            })
            .collect())
    }
}

/// Walks the `outlet.3.1.<column>` table column into an index→value map. The
/// column id is the arc immediately after `base`, the outlet index the one
/// after that.
async fn walk_column(
    session: &mut dyn SnmpQuery,
    base: &[u64],
    column: u64,
) -> Result<BTreeMap<u64, String>> {
    let column_base: Vec<u64> = base
        .iter()
        .copied()
        .chain(std::iter::once(column))
        .collect();
    let mut map = BTreeMap::new();
    for (oid, value) in session.walk(&column_base).await? {
        if let (Some(index), Some(text)) = (
            table_index(&oid, &column_base),
            value.as_str().filter(|s| !s.is_empty()),
        ) {
            map.insert(index, text);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::RaritanMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_raritan_pdu2() {
        assert!(RaritanMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.13742.6.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RaritanMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    fn raritan_session() -> WalkSession {
        WalkSession::parse(
            ".1.3.6.1.4.1.13742.6.3.2.1.1.2.1 = STRING: \"Raritan\"\n\
             .1.3.6.1.4.1.13742.6.3.2.1.1.3.1 = STRING: \"PX3-5260V\"\n\
             .1.3.6.1.4.1.13742.6.3.2.1.1.4.1 = STRING: \"QFG7100123\"\n\
             .1.3.6.1.4.1.13742.6.3.2.1.1.6.1 = STRING: \"16A\"\n\
             .1.3.6.1.4.1.13742.6.3.2.2.1.13.1 = STRING: \"rack-pdu-7\"\n\
             .1.3.6.1.4.1.13742.6.3.5.3.1.2.1 = STRING: \"1\"\n\
             .1.3.6.1.4.1.13742.6.3.5.3.1.2.2 = STRING: \"2\"\n\
             .1.3.6.1.4.1.13742.6.3.5.3.1.3.1 = STRING: \"Server A\"\n\
             .1.3.6.1.4.1.13742.6.3.5.3.1.29.1 = STRING: \"IEC 60320 C13\"\n\
             .1.3.6.1.4.1.13742.6.3.5.3.1.29.2 = STRING: \"IEC 60320 C19\"\n",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn older_glpi_is_networking_without_plugs() {
        let mut session = raritan_session();
        let mut device = NetworkDevice {
            glpi_version: Some("10.0.19".to_owned()),
            ..NetworkDevice::default()
        };
        RaritanMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Raritan"));
        assert_eq!(device.info.model.as_deref(), Some("PX3-5260V"));
        assert_eq!(device.info.serial.as_deref(), Some("QFG7100123"));
        assert_eq!(device.info.name.as_deref(), Some("rack-pdu-7"));
        assert!(device.pdu.is_none());
    }

    #[tokio::test]
    async fn glpi12_reports_pdu_with_plugs() {
        let mut session = raritan_session();
        let mut device = NetworkDevice {
            glpi_version: Some("12.0.0".to_owned()),
            ..NetworkDevice::default()
        };
        RaritanMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("PDU"));

        let pdu = device.pdu.as_ref().unwrap();
        assert_eq!(pdu.model.as_deref(), Some("16A"));
        assert_eq!(pdu.plugs.len(), 2);
        // Outlet 1: name set, descriptor C13.
        assert_eq!(pdu.plugs[0].number, 1);
        assert_eq!(pdu.plugs[0].name.as_deref(), Some("Server A"));
        assert_eq!(pdu.plugs[0].r#type, "IEC 60320 C13");
        // Outlet 2: no name -> label; descriptor C19.
        assert_eq!(pdu.plugs[1].number, 2);
        assert_eq!(pdu.plugs[1].name.as_deref(), Some("2"));
        assert_eq!(pdu.plugs[1].r#type, "IEC 60320 C19");
    }
}
