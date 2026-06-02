// SPDX-License-Identifier: GPL-2.0-only

//! UPS vendor MIB support (APC, standard UPS-MIB, Riello).
//!
//! Applies to APC PowerNet (`1.3.6.1.4.1.318`), the standard UPS-MIB
//! (`1.3.6.1.2.1.33`) and Riello (`1.3.6.1.4.1.5491`) devices. Fills the
//! manufacturer, model, serial and firmware, preferring the Riello private
//! group on Riello hardware and otherwise the standard UPS-MIB with an APC
//! fallback. Ported from the upstream `GLPI::Agent::SNMP::MibSupport::UPS`; the
//! type is reported as `NETWORKING` pending server-side `POWER` support.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// APC PowerNet enterprise OID.
const APC: &str = "1.3.6.1.4.1.318";
/// Standard `UPS-MIB`.
const UPS_MIB: &str = "1.3.6.1.2.1.33";
/// Riello enterprise OID.
const RIELLO: &str = "1.3.6.1.4.1.5491";

/// `rupsIdentManufacturer` (`riello.10.1.1.1.0`).
const RUPS_IDENT_MANUFACTURER: [u64; 12] = [1, 3, 6, 1, 4, 1, 5491, 10, 1, 1, 1, 0];
/// `rupsIdentModel` (`riello.10.1.1.2.0`).
const RUPS_IDENT_MODEL: [u64; 12] = [1, 3, 6, 1, 4, 1, 5491, 10, 1, 1, 2, 0];
/// `rupsIdentUPSSoftwareVersion` (`riello.10.1.1.3.0`).
const RUPS_IDENT_SOFTWARE: [u64; 12] = [1, 3, 6, 1, 4, 1, 5491, 10, 1, 1, 3, 0];
/// `upsAdvIdentSerialNumber` (`apc.1.1.1.1.2.3.0`).
const UPS_ADV_IDENT_SERIAL: [u64; 14] = [1, 3, 6, 1, 4, 1, 318, 1, 1, 1, 1, 2, 3, 0];
/// `sPDUIdentFirmwareRev` (`apc.1.1.4.1.2.0`).
const SPDU_IDENT_FIRMWARE: [u64; 13] = [1, 3, 6, 1, 4, 1, 318, 1, 1, 4, 1, 2, 0];
/// `sPDUIdentModelNumber` (`apc.1.1.4.1.4.0`).
const SPDU_IDENT_MODEL: [u64; 13] = [1, 3, 6, 1, 4, 1, 318, 1, 1, 4, 1, 4, 0];
/// `sPDUIdentSerialNumber` (`apc.1.1.4.1.5.0`).
const SPDU_IDENT_SERIAL: [u64; 13] = [1, 3, 6, 1, 4, 1, 318, 1, 1, 4, 1, 5, 0];
/// `upsIdentManufacturer` (`upsMIB.1.1.1.0`).
const UPS_IDENT_MANUFACTURER: [u64; 11] = [1, 3, 6, 1, 2, 1, 33, 1, 1, 1, 0];
/// `upsIdentModel` (`upsMIB.1.1.2.0`).
const UPS_IDENT_MODEL: [u64; 11] = [1, 3, 6, 1, 2, 1, 33, 1, 1, 2, 0];
/// `upsIdentUPSSoftwareVersion` (`upsMIB.1.1.3.0`).
const UPS_IDENT_SOFTWARE: [u64; 11] = [1, 3, 6, 1, 2, 1, 33, 1, 1, 3, 0];

/// Vendor MIB module for UPS devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct UpsMib;

#[async_trait]
impl MibSupport for UpsMib {
    fn name(&self) -> &'static str {
        "ups"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let oid = info.sys_object_id.as_deref();
        sysobjectid_matches(oid, APC)
            || sysobjectid_matches(oid, UPS_MIB)
            || sysobjectid_matches(oid, RIELLO)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let is_riello = sysobjectid_matches(device.info.sys_object_id.as_deref(), RIELLO);

        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            let riello = if is_riello {
                get_string(session, &RUPS_IDENT_MANUFACTURER).await?
            } else {
                None
            };
            device.info.manufacturer = match riello {
                Some(m) => Some(m),
                None => get_string(session, &UPS_IDENT_MANUFACTURER).await?,
            };
        }
        if device.info.model.is_none() {
            let riello = if is_riello {
                get_string(session, &RUPS_IDENT_MODEL).await?
            } else {
                None
            };
            device.info.model = match riello {
                Some(m) => Some(m),
                None => first_of(session, &[&UPS_IDENT_MODEL, &SPDU_IDENT_MODEL]).await?,
            };
        }
        if device.info.serial.is_none() {
            device.info.serial =
                first_of(session, &[&UPS_ADV_IDENT_SERIAL, &SPDU_IDENT_SERIAL]).await?;
        }
        if device.info.firmware.is_none() {
            let riello = if is_riello {
                get_string(session, &RUPS_IDENT_SOFTWARE).await?
            } else {
                None
            };
            device.info.firmware = match riello {
                Some(f) => Some(f),
                None => first_of(session, &[&UPS_IDENT_SOFTWARE, &SPDU_IDENT_FIRMWARE]).await?,
            };
        }
        Ok(())
    }
}

/// Returns the first non-empty string among `oids`, in order.
async fn first_of(session: &mut dyn SnmpQuery, oids: &[&[u64]]) -> Result<Option<String>> {
    for oid in oids {
        if let Some(value) = get_string(session, oid).await? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::UpsMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    fn device(oid: &str) -> NetworkDevice {
        NetworkDevice {
            info: DeviceInfo {
                sys_object_id: Some(oid.to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        }
    }

    #[test]
    fn applies_to_apc_upsmib_and_riello() {
        for oid in [
            "1.3.6.1.4.1.318.1",
            "1.3.6.1.2.1.33.1",
            "1.3.6.1.4.1.5491.1",
        ] {
            assert!(UpsMib.applies_to(&device(oid).info));
        }
        assert!(!UpsMib.applies_to(&device("1.3.6.1.4.1.9").info));
    }

    #[tokio::test]
    async fn reads_standard_ups_mib() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.2.1.33.1.1.1.0 = STRING: \"EATON\"\n\
             .1.3.6.1.2.1.33.1.1.2.0 = STRING: \"5PX1500\"\n\
             .1.3.6.1.2.1.33.1.1.3.0 = STRING: \"1.08\"\n\
             .1.3.6.1.4.1.318.1.1.1.1.2.3.0 = STRING: \"5PX1234567\"\n",
        )
        .unwrap();
        let mut device = device("1.3.6.1.2.1.33.2");
        UpsMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("EATON"));
        assert_eq!(device.info.model.as_deref(), Some("5PX1500"));
        assert_eq!(device.info.firmware.as_deref(), Some("1.08"));
        assert_eq!(device.info.serial.as_deref(), Some("5PX1234567"));
    }

    #[tokio::test]
    async fn prefers_riello_private_group() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.5491.10.1.1.1.0 = STRING: \"Riello\"\n\
             .1.3.6.1.4.1.5491.10.1.1.2.0 = STRING: \"Sentinel Dual\"\n\
             .1.3.6.1.4.1.5491.10.1.1.3.0 = STRING: \"4.2\"\n",
        )
        .unwrap();
        let mut device = device("1.3.6.1.4.1.5491.1");
        UpsMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Riello"));
        assert_eq!(device.info.model.as_deref(), Some("Sentinel Dual"));
        assert_eq!(device.info.firmware.as_deref(), Some("4.2"));
    }
}
