// SPDX-License-Identifier: GPL-2.0-only

//! Cisco Meraki vendor MIB support (networking).
//!
//! Applies to Cisco Meraki products (`MERAKI-CLOUD-CONTROLLER-MIB`,
//! `1.3.6.1.4.1.29671.2`). Sets the `NETWORKING` type and manufacturer, and
//! derives the model from the system description (`Meraki <model>`, otherwise
//! the description with a trailing `, Modular Uplinks` removed). Ported from the
//! upstream `GLPI::Agent::SNMP::MibSupport::CiscoMeraki`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `merakiProducts` (`meraki.2`).
const MERAKI_PRODUCTS: &str = "1.3.6.1.4.1.29671.2";

/// Vendor MIB module for Cisco Meraki devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct CiscoMerakiMib;

#[async_trait]
impl MibSupport for CiscoMerakiMib {
    fn name(&self) -> &'static str {
        "cisco-meraki"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), MERAKI_PRODUCTS)
    }

    async fn run(&self, _session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Cisco Meraki".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = device.info.description.as_deref().map(model_from_descr);
        }
        Ok(())
    }
}

/// Model from the description: the token after a leading `Meraki`, else the
/// description with a trailing `, Modular Uplinks` stripped.
fn model_from_descr(descr: &str) -> String {
    if let Some(rest) = descr
        .strip_prefix("Meraki ")
        .or_else(|| descr.strip_prefix("meraki "))
    {
        if let Some(word) = rest.split_whitespace().next() {
            return word.to_owned();
        }
    }
    descr
        .strip_suffix(", Modular Uplinks")
        .unwrap_or(descr)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::CiscoMerakiMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_meraki_products() {
        assert!(CiscoMerakiMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.29671.2.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!CiscoMerakiMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.29671.1".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn derives_model_from_description() {
        let mut session = WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"x\"\n").unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("Meraki MS220-8P Cloud Managed Switch".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        CiscoMerakiMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Cisco Meraki"));
        assert_eq!(device.info.model.as_deref(), Some("MS220-8P"));
    }

    #[tokio::test]
    async fn strips_modular_uplinks_suffix() {
        let mut session = WalkSession::parse(".1.3.6.1.2.1.1.1.0 = STRING: \"x\"\n").unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("MS425-32, Modular Uplinks".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        CiscoMerakiMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("MS425-32"));
    }
}
