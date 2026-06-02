// SPDX-License-Identifier: GPL-2.0-only

//! Siemens SICAM vendor MIB support (networking).
//!
//! Applies to Siemens SICAM devices (`SIEMENS-SMI`, `1.3.6.1.4.1.22638`). The
//! identity is parsed from the comma-separated `Siemens AG, …` system
//! description: model (fields 1–2), hardware revision (field 3), `FW:` firmware
//! and `SN:` serial. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::SiemensSicam`; the component/product tree is
//! not modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Siemens SICAM enterprise OID.
const SIEMENS: &str = "1.3.6.1.4.1.22638";

/// Vendor MIB module for Siemens SICAM devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct SiemensSicamMib;

#[async_trait]
impl MibSupport for SiemensSicamMib {
    fn name(&self) -> &'static str {
        "siemens_sicam"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), SIEMENS)
    }

    async fn run(&self, _session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Siemens".to_owned());
        }

        let Some(description) = device.info.description.clone() else {
            return Ok(());
        };
        if !description.starts_with("Siemens AG,") {
            return Ok(());
        }
        let fields: Vec<&str> = description.split(',').map(str::trim).collect();

        if device.info.model.is_none() {
            if let (Some(a), Some(b)) = (fields.get(1), fields.get(2)) {
                device.info.model = Some(format!("{a} {b}"));
            }
        }
        if device.info.firmware.is_none() {
            device.info.firmware = fields
                .get(4)
                .and_then(|f| f.strip_prefix("FW: "))
                .map(str::to_owned);
        }
        if device.info.serial.is_none() {
            device.info.serial = fields
                .get(5)
                .and_then(|f| f.strip_prefix("SN: "))
                .map(str::to_owned);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SiemensSicamMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_siemens_sicam() {
        assert!(SiemensSicamMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.22638.11".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!SiemensSicamMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.22639".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn parses_description_fields() {
        let mut session = WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"x\"\n").unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some(
                    "Siemens AG, SICAM A8000, CP-8050, HW3, FW: 4.70, SN: BF1234567".to_owned(),
                ),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        SiemensSicamMib
            .run(&mut session, &mut device)
            .await
            .unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Siemens"));
        assert_eq!(device.info.model.as_deref(), Some("SICAM A8000 CP-8050"));
        assert_eq!(device.info.firmware.as_deref(), Some("4.70"));
        assert_eq!(device.info.serial.as_deref(), Some("BF1234567"));
    }
}
