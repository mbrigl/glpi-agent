// SPDX-License-Identifier: GPL-2.0-only

//! Dell iDRAC vendor MIB support.
//!
//! Applies to Dell iDRAC controllers (`1.3.6.1.4.1.674.10892`) and fills the
//! chassis serial (service tag). Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Idrac`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// iDRAC OID (`dell.10892`).
const IDRAC: &str = "1.3.6.1.4.1.674.10892";
/// `serial` (`idrac.2.1.1.11.0`) — the chassis service tag.
const SERIAL: [u64; 13] = [1, 3, 6, 1, 4, 1, 674, 10892, 2, 1, 1, 11, 0];

/// Vendor MIB module for Dell iDRAC controllers.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdracMib;

#[async_trait]
impl MibSupport for IdracMib {
    fn name(&self) -> &'static str {
        "idrac"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), IDRAC)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::IdracMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_idrac() {
        assert!(IdracMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.674.10892.5".to_owned()),
            ..DeviceInfo::default()
        }));
        // Plain Dell enterprise without the iDRAC arc must not match.
        assert!(!IdracMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.674.10893".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_service_tag_serial() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.674.10892.2.1.1.11.0 = STRING: \"7ABC123\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        IdracMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("7ABC123"));
    }
}
