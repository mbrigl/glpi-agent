// SPDX-License-Identifier: GPL-2.0-only

//! Kyocera printer vendor MIB support.
//!
//! Applies to Kyocera printers (`kyoceraPrinter`, `1.3.6.1.4.1.1347.41`) and
//! sets the SNMP hostname from the Kyocera private `sysName` scalar. Ported from
//! the upstream `GLPI::Agent::SNMP::MibSupport::Kyocera`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `kyoceraPrinter` (`kyocera.41`) — the sysObjectID prefix Kyocera printers use.
const KYOCERA_PRINTER: &str = "1.3.6.1.4.1.1347.41";
/// Kyocera private `sysName` (`kyocera.40.10.1.1.5.1`).
const SYS_NAME: [u64; 13] = [1, 3, 6, 1, 4, 1, 1347, 40, 10, 1, 1, 5, 1];

/// Vendor MIB module for Kyocera printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct KyoceraMib;

#[async_trait]
impl MibSupport for KyoceraMib {
    fn name(&self) -> &'static str {
        "kyocera"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), KYOCERA_PRINTER)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.name.is_none() {
            device.info.name = get_string(session, &SYS_NAME).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::KyoceraMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_kyocera_printer() {
        assert!(KyoceraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1347.41.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!KyoceraMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1347.40".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn sets_hostname() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.1347.40.10.1.1.5.1 = STRING: \"KM-ACCOUNTING\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        KyoceraMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.name.as_deref(), Some("KM-ACCOUNTING"));
    }
}
