// SPDX-License-Identifier: GPL-2.0-only

//! Epson printer vendor MIB support.
//!
//! Applies to Epson printers (`1.3.6.1.4.1.1248`) and fills the model and
//! serial number. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Epson` (the firmware-table and cartridge
//! enumeration in the upstream `run` is not modelled here).

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Epson enterprise OID.
const EPSON_ENTERPRISE: &str = "1.3.6.1.4.1.1248";
/// Epson model OID (`epson.1.2.2.1.1.1.2.1`).
const EPSON_MODEL: [u64; 15] = [1, 3, 6, 1, 4, 1, 1248, 1, 2, 2, 1, 1, 1, 2, 1];
/// Epson serial OID (`epson.1.2.2.1.1.1.5.1`).
const EPSON_SERIAL: [u64; 15] = [1, 3, 6, 1, 4, 1, 1248, 1, 2, 2, 1, 1, 1, 5, 1];

/// Vendor MIB module for Epson printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct EpsonMib;

#[async_trait]
impl MibSupport for EpsonMib {
    fn name(&self) -> &'static str {
        "epson"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), EPSON_ENTERPRISE)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = get_string(session, &EPSON_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &EPSON_SERIAL).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::EpsonMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_epson() {
        assert!(EpsonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.1248.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!EpsonMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_model_and_serial() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.1248.1.2.2.1.1.1.2.1 = STRING: \"WF-C5790\"\n\
             .1.3.6.1.4.1.1248.1.2.2.1.1.1.5.1 = STRING: \"X4PY123456\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        EpsonMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("WF-C5790"));
        assert_eq!(device.info.serial.as_deref(), Some("X4PY123456"));
    }
}
