// SPDX-License-Identifier: GPL-2.0-only

//! Nokia / Alcatel vendor MIB support (networking).
//!
//! Applies to Alcatel (`1.3.6.1.4.1.637`) and Nokia TiMOS (`1.3.6.1.4.1.6527`)
//! devices. Derives manufacturer, model and firmware from the system
//! description and resolves the chassis serial from the ISAM equipment table or,
//! failing that, the TIMETRA chassis hardware table. The recursive hardware
//! component tree of the upstream `GLPI::Agent::SNMP::MibSupport::Nokia` is not
//! modelled here; only the scalar identity is ported.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    as_number, instance_suffix, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Alcatel enterprise OID.
const ALCATEL: &str = "1.3.6.1.4.1.637";
/// Nokia TIMETRA enterprise OID.
const TIMETRA: &str = "1.3.6.1.4.1.6527";
/// `eqptHolderSerialNumber` (`asamEquipmentMIB.2.1.13`) — ISAM holder serials.
const EQPT_HOLDER_SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 637, 61, 1, 23, 2, 1, 13];
/// `tmnxHwSerialNumber` (`tmnxHwEntry.5.1`).
const TMNX_HW_SERIAL_NUMBER: [u64; 16] = [1, 3, 6, 1, 4, 1, 6527, 3, 1, 2, 2, 1, 8, 1, 5, 1];
/// `tmnxHwContainedIn` (`tmnxHwEntry.13.1`) — parent index per hardware row.
const TMNX_HW_CONTAINED_IN: [u64; 16] = [1, 3, 6, 1, 4, 1, 6527, 3, 1, 2, 2, 1, 8, 1, 13, 1];

/// Vendor MIB module for Nokia / Alcatel devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct NokiaMib;

#[async_trait]
impl MibSupport for NokiaMib {
    fn name(&self) -> &'static str {
        "nokia"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, ALCATEL) || sysobjectid_matches(oid, TIMETRA)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let description = device.info.description.clone().unwrap_or_default();
        let is_nokia = description.contains("Nokia");

        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(if is_nokia { "Nokia" } else { "Alcatel" }.to_owned());
        }
        if device.info.model.is_none() && is_nokia {
            device.info.model = model_from_description(&description);
        }
        if device.info.firmware.is_none() && is_nokia {
            device.info.firmware = description.split_whitespace().next().map(str::to_owned);
        }
        if device.info.serial.is_none() {
            device.info.serial = self.serial(session).await?;
        }
        Ok(())
    }
}

impl NokiaMib {
    /// Resolves the chassis serial: the first valid ISAM holder serial, else the
    /// TIMETRA hardware row that is not contained in any other (the chassis).
    async fn serial(&self, session: &mut dyn SnmpQuery) -> Result<Option<String>> {
        let holders = session.walk(&EQPT_HOLDER_SERIAL_NUMBER).await?;
        let mut isam: Vec<(Vec<u64>, String)> = holders
            .into_iter()
            .filter_map(|(oid, value)| {
                let suffix = instance_suffix(&oid, &EQPT_HOLDER_SERIAL_NUMBER)?;
                let serial = value.as_str().filter(|s| !s.is_empty())?;
                (!serial.eq_ignore_ascii_case("NOT AVAILABLE")).then_some((suffix, serial))
            })
            .collect();
        isam.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some((_, serial)) = isam.into_iter().next() {
            return Ok(Some(serial));
        }

        // Chassis = the hardware row whose tmnxHwContainedIn value is 0.
        let mut contained: BTreeMap<Vec<u64>, i64> = BTreeMap::new();
        for (oid, value) in session.walk(&TMNX_HW_CONTAINED_IN).await? {
            if let (Some(suffix), Some(parent)) = (
                instance_suffix(&oid, &TMNX_HW_CONTAINED_IN),
                as_number(&value),
            ) {
                contained.insert(suffix, parent);
            }
        }
        let Some((chassis, _)) = contained.iter().find(|(_, &parent)| parent == 0) else {
            return Ok(None);
        };
        let oid: Vec<u64> = TMNX_HW_SERIAL_NUMBER
            .iter()
            .chain(chassis)
            .copied()
            .collect();
        Ok(session
            .get(&oid)
            .await?
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty()))
    }
}

/// Extracts the model between the "Nokia" vendor token and "Copyright".
fn model_from_description(description: &str) -> Option<String> {
    let lower = description.to_lowercase();
    let start = lower.find("nokia")? + "nokia".len();
    let end = lower[start..].find("copyright")? + start;
    let model = description[start..end].trim();
    (!model.is_empty()).then(|| model.to_owned())
}

#[cfg(test)]
mod tests {
    use super::NokiaMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_to_alcatel_and_timetra() {
        assert!(NokiaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.6527.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(NokiaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.637.61".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!NokiaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn derives_identity_and_chassis_serial() {
        // No ISAM holders; chassis is the row contained in 0.
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.6527.3.1.2.2.1.8.1.13.1.1 = INTEGER: 0\n\
             .1.3.6.1.4.1.6527.3.1.2.2.1.8.1.13.1.2 = INTEGER: 1\n\
             .1.3.6.1.4.1.6527.3.1.2.2.1.8.1.5.1.1 = STRING: \"NS1234567890\"\n\
             .1.3.6.1.4.1.6527.3.1.2.2.1.8.1.5.1.2 = STRING: \"CARD-SN-2\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some(
                    "TiMOS-B-20.10.R1 Nokia 7750 SR-7 Copyright (c) 2000-2021 Nokia.".to_owned(),
                ),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        NokiaMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Nokia"));
        assert_eq!(device.info.model.as_deref(), Some("7750 SR-7"));
        assert_eq!(device.info.firmware.as_deref(), Some("TiMOS-B-20.10.R1"));
        assert_eq!(device.info.serial.as_deref(), Some("NS1234567890"));
    }

    #[tokio::test]
    async fn prefers_isam_holder_serial_and_skips_not_available() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.637.61.1.23.2.1.13.1 = STRING: \"NOT AVAILABLE\"\n\
             .1.3.6.1.4.1.637.61.1.23.2.1.13.2 = STRING: \"ISAM-SN-42\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("ISAM 7302 Alcatel-Lucent".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        NokiaMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Alcatel"));
        assert_eq!(device.info.serial.as_deref(), Some("ISAM-SN-42"));
    }
}
