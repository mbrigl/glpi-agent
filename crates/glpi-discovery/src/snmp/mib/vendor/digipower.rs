// SPDX-License-Identifier: GPL-2.0-only

//! DigiPower PDU vendor MIB support.
//!
//! Applies to DigiPower devices (`DigiPower-PDU-MIB`, `1.3.6.1.4.1.17420`) and
//! fills the type (`PDU` on GLPI 12+, else `NETWORKING`), firmware
//! (`devVersion`) and model (`pdu01ModelNo`). The device-level MAC reported by
//! the upstream module is not modelled here. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::DigiPower`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, pdu_type, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// DigiPower enterprise OID.
const DIGIPOWER: &str = "1.3.6.1.4.1.17420";
/// `devVersion` (`digipower.1.2.4.0`).
const DEV_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 17420, 1, 2, 4, 0];
/// `pdu01ModelNo` (`digipower.1.2.9.1.19.0`).
const PDU01_MODEL_NO: [u64; 13] = [1, 3, 6, 1, 4, 1, 17420, 1, 2, 9, 1, 19, 0];

/// Vendor MIB module for DigiPower PDUs.
#[derive(Debug, Default, Clone, Copy)]
pub struct DigiPowerMib;

#[async_trait]
impl MibSupport for DigiPowerMib {
    fn name(&self) -> &'static str {
        "digipower"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), DIGIPOWER)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some(pdu_type(device.glpi_version.as_deref()).to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &DEV_VERSION).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PDU01_MODEL_NO).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DigiPowerMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_digipower() {
        assert!(DigiPowerMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.17420.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!DigiPowerMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.17421".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_firmware_and_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.17420.1.2.4.0 = STRING: \"V1.6.2\"\n\
             .1.3.6.1.4.1.17420.1.2.9.1.19.0 = STRING: \"PDU-1606\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        DigiPowerMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.firmware.as_deref(), Some("V1.6.2"));
        assert_eq!(device.info.model.as_deref(), Some("PDU-1606"));
    }
}
