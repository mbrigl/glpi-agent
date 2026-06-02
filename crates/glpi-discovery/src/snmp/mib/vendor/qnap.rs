// SPDX-License-Identifier: GPL-2.0-only

//! QNAP vendor MIB support.
//!
//! Applies to QNAP storage appliances under the QNAP enterprise
//! (`1.3.6.1.4.1.24681`). Sets the manufacturer and `STORAGE` type and fills
//! the model from `NAS-MIB::es_ModelName`. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Qnap`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// QNAP storage enterprise OID.
const QNAP_ENTERPRISE: &str = "1.3.6.1.4.1.24681";
/// `NAS-MIB::es_ModelName.0` (`qnap_storage.2.2.12.0`).
const ES_MODEL_NAME: [u64; 11] = [1, 3, 6, 1, 4, 1, 24681, 2, 2, 12, 0];

/// Vendor MIB module for QNAP storage appliances.
#[derive(Debug, Default, Clone, Copy)]
pub struct QnapMib;

#[async_trait]
impl MibSupport for QnapMib {
    fn name(&self) -> &'static str {
        "qnap"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), QNAP_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Qnap".to_owned());
        }
        if device.info.r#type.is_none() {
            device.info.r#type = Some("STORAGE".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &ES_MODEL_NAME).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::QnapMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_qnap() {
        assert!(QnapMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.24681.2".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!QnapMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn sets_manufacturer_type_and_model() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.24681.2.2.12.0 = STRING: \"TS-453D\"\n").unwrap();
        let mut device = NetworkDevice::default();
        QnapMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Qnap"));
        assert_eq!(device.info.r#type.as_deref(), Some("STORAGE"));
        assert_eq!(device.info.model.as_deref(), Some("TS-453D"));
    }
}
