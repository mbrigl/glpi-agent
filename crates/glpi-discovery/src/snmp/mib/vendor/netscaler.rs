// SPDX-License-Identifier: GPL-2.0-only

//! Citrix NetScaler vendor MIB support.
//!
//! Applies to Citrix NetScaler devices (`NS-ROOT-MIB`, `1.3.6.1.4.1.5951`) and
//! fills the chassis serial number from `sysHardwareSerialNumber`. Ported from
//! the upstream `GLPI::Agent::SNMP::MibSupport::CitrixNetscaler`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `NS-ROOT-MIB` (Citrix NetScaler) enterprise OID.
const NETSCALER_ENTERPRISE: &str = "1.3.6.1.4.1.5951";
/// `sysHardwareSerialNumber.0`.
const SYS_HARDWARE_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 5951, 4, 1, 1, 14, 0];

/// Vendor MIB module for Citrix NetScaler devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct NetscalerMib;

#[async_trait]
impl MibSupport for NetscalerMib {
    fn name(&self) -> &'static str {
        "citrix-netscaler"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), NETSCALER_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SYS_HARDWARE_SERIAL_NUMBER).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NetscalerMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_netscaler() {
        assert!(NetscalerMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.5951.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!NetscalerMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_serial() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.5951.4.1.1.14.0 = STRING: \"XYZ1234567\"\n").unwrap();
        let mut device = NetworkDevice::default();
        NetscalerMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.serial.as_deref(), Some("XYZ1234567"));
    }
}
