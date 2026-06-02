// SPDX-License-Identifier: GPL-2.0-only

//! Raritan PDU vendor MIB support.
//!
//! Applies to Raritan PDU2 devices (`PDU2-MIB`, `1.3.6.1.4.1.13742.6`). Fills
//! the manufacturer, model and serial number from the nameplate entry. Ported
//! from the upstream `GLPI::Agent::SNMP::MibSupport::Raritan` (the per-outlet
//! plug enumeration is not modelled here).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Raritan `pdu2` OID (`raritan.6`).
const RARITAN_PDU2: &str = "1.3.6.1.4.1.13742.6";
/// `pduManufacturer` (`nameplateEntry.2.1`).
const PDU_MANUFACTURER: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 2, 1];
/// `pduModel` (`nameplateEntry.3.1`).
const PDU_MODEL: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 3, 1];
/// `pduSerialNumber` (`nameplateEntry.4.1`).
const PDU_SERIAL_NUMBER: [u64; 14] = [1, 3, 6, 1, 4, 1, 13742, 6, 3, 2, 1, 1, 4, 1];

/// Vendor MIB module for Raritan PDUs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RaritanMib;

#[async_trait]
impl MibSupport for RaritanMib {
    fn name(&self) -> &'static str {
        "raritan-pdu2"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), RARITAN_PDU2)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(
                get_string(session, &PDU_MANUFACTURER)
                    .await?
                    .unwrap_or_else(|| "Raritan".to_owned()),
            );
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PDU_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &PDU_SERIAL_NUMBER).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RaritanMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_raritan_pdu2() {
        assert!(RaritanMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.13742.6.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!RaritanMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_manufacturer_model_and_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.13742.6.3.2.1.1.2.1 = STRING: \"Raritan\"\n\
             .1.3.6.1.4.1.13742.6.3.2.1.1.3.1 = STRING: \"PX3-5260V\"\n\
             .1.3.6.1.4.1.13742.6.3.2.1.1.4.1 = STRING: \"QFG7100123\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        RaritanMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Raritan"));
        assert_eq!(device.info.model.as_deref(), Some("PX3-5260V"));
        assert_eq!(device.info.serial.as_deref(), Some("QFG7100123"));
    }
}
