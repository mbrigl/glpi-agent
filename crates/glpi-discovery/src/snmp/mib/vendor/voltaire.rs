// SPDX-License-Identifier: GPL-2.0-only

//! Voltaire (InfiniBand) vendor MIB support (networking).
//!
//! Applies to Voltaire devices (`1.3.6.1.4.1.5206`) and sets the `NETWORKING`
//! type, manufacturer, model (the `sysName` up to the first dash), serial and
//! firmware. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Voltaire`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Voltaire enterprise OID.
const VOLTAIRE: &str = "1.3.6.1.4.1.5206";
/// `sysName.0`.
const SYS_NAME: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 5, 0];
/// `serialnumber` (`voltaire.3.29.1.3.1007.1`).
const SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 5206, 3, 29, 1, 3, 1007, 1];
/// `version` (`voltaire.3.1.0`).
const VERSION: [u64; 10] = [1, 3, 6, 1, 4, 1, 5206, 3, 1, 0];

/// Vendor MIB module for Voltaire devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct VoltaireMib;

#[async_trait]
impl MibSupport for VoltaireMib {
    fn name(&self) -> &'static str {
        "voltaire"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), VOLTAIRE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Voltaire".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &SYS_NAME)
                .await?
                .map(|name| name.split('-').next().unwrap_or(&name).to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &VERSION).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::VoltaireMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_voltaire() {
        assert!(VoltaireMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.5206.3".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!VoltaireMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.5207".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_extracts_model() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.2.1.1.5.0 = STRING: \"ISR9024D-001122\"\n\
             .1.3.6.1.4.1.5206.3.29.1.3.1007.1 = STRING: \"VLT123456\"\n\
             .1.3.6.1.4.1.5206.3.1.0 = STRING: \"7.4.4\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        VoltaireMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Voltaire"));
        assert_eq!(device.info.model.as_deref(), Some("ISR9024D"));
        assert_eq!(device.info.serial.as_deref(), Some("VLT123456"));
        assert_eq!(device.info.firmware.as_deref(), Some("7.4.4"));
    }
}
