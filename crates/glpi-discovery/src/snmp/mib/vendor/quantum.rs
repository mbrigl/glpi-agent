// SPDX-License-Identifier: GPL-2.0-only

//! Quantum vendor MIB support.
//!
//! Applies to Quantum storage devices (`1.3.6.1.4.1.3764`). Sets the `STORAGE`
//! type and fills the manufacturer, model, serial number and firmware (the SNMP
//! agent version) from `productAgentInfo`. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Quantum` (the component enumeration is not
//! modelled here).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Quantum enterprise OID.
const QUANTUM_ENTERPRISE: &str = "1.3.6.1.4.1.3764";
/// `productSnmpAgentVersion.0` (`productAgentInfo.2.0`).
const PRODUCT_SNMP_AGENT_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 3764, 1, 1, 10, 2, 0];
/// `productName.0` (`productAgentInfo.3.0`).
const PRODUCT_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 3764, 1, 1, 10, 3, 0];
/// `productVendor.0` (`productAgentInfo.6.0`).
const PRODUCT_VENDOR: [u64; 12] = [1, 3, 6, 1, 4, 1, 3764, 1, 1, 10, 6, 0];
/// `productSerialNumber.0` (`productAgentInfo.10.0`).
const PRODUCT_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 4, 1, 3764, 1, 1, 10, 10, 0];

/// Vendor MIB module for Quantum storage devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct QuantumMib;

#[async_trait]
impl MibSupport for QuantumMib {
    fn name(&self) -> &'static str {
        "quantum"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), QUANTUM_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("STORAGE".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = get_string(session, &PRODUCT_VENDOR).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &PRODUCT_NAME).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &PRODUCT_SERIAL_NUMBER).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &PRODUCT_SNMP_AGENT_VERSION).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::QuantumMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_quantum() {
        assert!(QuantumMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.3764.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!QuantumMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.3764.1.1.10.2.0 = STRING: \"7.0.1\"\n\
             .1.3.6.1.4.1.3764.1.1.10.3.0 = STRING: \"Scalar i6\"\n\
             .1.3.6.1.4.1.3764.1.1.10.6.0 = STRING: \"Quantum\"\n\
             .1.3.6.1.4.1.3764.1.1.10.10.0 = STRING: \"QTM123456\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        QuantumMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("STORAGE"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Quantum"));
        assert_eq!(device.info.model.as_deref(), Some("Scalar i6"));
        assert_eq!(device.info.serial.as_deref(), Some("QTM123456"));
        assert_eq!(device.info.firmware.as_deref(), Some("7.0.1"));
    }
}
