// SPDX-License-Identifier: GPL-2.0-only

//! Voltronic UPS vendor MIB support.
//!
//! Applies to Voltronic-based devices (`1.3.6.1.4.1.43943`) and fills the model,
//! serial, firmware (with a leading `VERFW:` stripped) and manufacturer. Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::Voltronic`; the type is
//! reported as `NETWORKING` pending server-side `POWER` support.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Voltronic enterprise OID.
const VOLTRONIC_MIB: &str = "1.3.6.1.4.1.43943";
/// `upsIdManufacturer` (`upsIdent.1.0`).
const UPS_ID_MANUFACTURER: [u64; 12] = [1, 3, 6, 1, 4, 1, 43943, 1, 1, 1, 1, 0];
/// `upsIdModelName` (`upsIdent.3.0`).
const UPS_ID_MODEL_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 43943, 1, 1, 1, 3, 0];
/// `upsIdSerialNumber` (`upsIdent.4.0`).
const UPS_ID_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 43943, 1, 1, 1, 4, 0];
/// `upsIdFWVersion` (`upsIdent.6.0`).
const UPS_ID_FW_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 43943, 1, 1, 1, 6, 0];

/// Vendor MIB module for Voltronic UPS devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct VoltronicMib;

#[async_trait]
impl MibSupport for VoltronicMib {
    fn name(&self) -> &'static str {
        "voltronic"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), VOLTRONIC_MIB)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(
                get_string(session, &UPS_ID_MANUFACTURER)
                    .await?
                    .unwrap_or_else(|| "Voltronic".to_owned()),
            );
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &UPS_ID_MODEL_NAME).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &UPS_ID_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &UPS_ID_FW_VERSION).await?.map(|fw| {
                fw.strip_prefix("VERFW:")
                    .or_else(|| fw.strip_prefix("verfw:"))
                    .unwrap_or(&fw)
                    .to_owned()
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::VoltronicMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_voltronic() {
        assert!(VoltronicMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.43943.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!VoltronicMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.43944".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_strips_verfw_prefix() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.43943.1.1.1.1.0 = STRING: \"Voltronic Power\"\n\
             .1.3.6.1.4.1.43943.1.1.1.3.0 = STRING: \"Cybertron 3000\"\n\
             .1.3.6.1.4.1.43943.1.1.1.4.0 = STRING: \"VPW0012345\"\n\
             .1.3.6.1.4.1.43943.1.1.1.6.0 = STRING: \"VERFW:01.02.03\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        VoltronicMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Voltronic Power"));
        assert_eq!(device.info.model.as_deref(), Some("Cybertron 3000"));
        assert_eq!(device.info.serial.as_deref(), Some("VPW0012345"));
        assert_eq!(device.info.firmware.as_deref(), Some("01.02.03"));
    }
}
