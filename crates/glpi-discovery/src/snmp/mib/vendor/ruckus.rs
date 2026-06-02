// SPDX-License-Identifier: GPL-2.0-only

//! Ruckus vendor MIB support.
//!
//! Applies to Ruckus products (`RUCKUS-ROOT-MIB`, products under
//! `1.3.6.1.4.1.25053.3`). Fills the model and serial from
//! `RUCKUS-CMN-HWINFO-MIB` and the firmware from `RUCKUS-CMN-SWINFO-MIB`.
//! Ported from the upstream `GLPI::Agent::SNMP::MibSupport::Ruckus`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `ruckusProducts` OID (`ruckusRootMIB.3`).
const RUCKUS_PRODUCTS: &str = "1.3.6.1.4.1.25053.3";
/// `ruckusHwInfoModelNumber.0`.
const RUCKUS_HW_INFO_MODEL_NUMBER: [u64; 15] = [1, 3, 6, 1, 4, 1, 25053, 1, 1, 2, 1, 1, 1, 1, 0];
/// `ruckusHwInfoSerialNumber.0`.
const RUCKUS_HW_INFO_SERIAL_NUMBER: [u64; 15] = [1, 3, 6, 1, 4, 1, 25053, 1, 1, 2, 1, 1, 1, 2, 0];
/// `ruckusSwRevision` (`ruckusSwInfo.1.1.3.1`).
const RUCKUS_SW_REVISION: [u64; 17] = [1, 3, 6, 1, 4, 1, 25053, 1, 1, 3, 1, 1, 1, 1, 1, 3, 1];

/// Vendor MIB module for Ruckus products.
#[derive(Debug, Default, Clone, Copy)]
pub struct RuckusMib;

#[async_trait]
impl MibSupport for RuckusMib {
    fn name(&self) -> &'static str {
        "ruckus"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), RUCKUS_PRODUCTS)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &RUCKUS_HW_INFO_MODEL_NUMBER).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &RUCKUS_HW_INFO_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &RUCKUS_SW_REVISION).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RuckusMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_ruckus_products() {
        assert!(RuckusMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.25053.3.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RuckusMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_serial_and_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.25053.1.1.2.1.1.1.1.0 = STRING: \"ZoneFlex R710\"\n\
             .1.3.6.1.4.1.25053.1.1.2.1.1.1.2.0 = STRING: \"301548000123\"\n\
             .1.3.6.1.4.1.25053.1.1.3.1.1.1.1.1.3.1 = STRING: \"5.2.0.0.699\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        RuckusMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("ZoneFlex R710"));
        assert_eq!(device.info.serial.as_deref(), Some("301548000123"));
        assert_eq!(device.info.firmware.as_deref(), Some("5.2.0.0.699"));
    }
}
