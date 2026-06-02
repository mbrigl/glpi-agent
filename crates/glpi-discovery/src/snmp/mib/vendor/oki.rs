// SPDX-License-Identifier: GPL-2.0-only

//! OKI printer vendor MIB support.
//!
//! Applies to OKI printers (`1.3.6.1.4.1.2001`) and fills the model and serial
//! number. Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Oki`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// OKI enterprise OID.
const OKI_ENTERPRISE: &str = "1.3.6.1.4.1.2001";
/// OKI model OID (`oki.1.1.1.1.11.1.10.25.0`).
const OKI_MODEL: [u64; 16] = [1, 3, 6, 1, 4, 1, 2001, 1, 1, 1, 1, 11, 1, 10, 25, 0];
/// OKI serial OID (`oki.1.1.1.1.11.1.10.45.0`).
const OKI_SERIAL: [u64; 16] = [1, 3, 6, 1, 4, 1, 2001, 1, 1, 1, 1, 11, 1, 10, 45, 0];

/// Vendor MIB module for OKI printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct OkiMib;

#[async_trait]
impl MibSupport for OkiMib {
    fn name(&self) -> &'static str {
        "oki"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), OKI_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &OKI_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &OKI_SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::OkiMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_oki() {
        assert!(OkiMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2001.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!OkiMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_and_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.2001.1.1.1.1.11.1.10.25.0 = STRING: \"MC563\"\n\
             .1.3.6.1.4.1.2001.1.1.1.1.11.1.10.45.0 = STRING: \"AK12345678\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        OkiMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("MC563"));
        assert_eq!(device.info.serial.as_deref(), Some("AK12345678"));
    }
}
