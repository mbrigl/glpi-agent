// SPDX-License-Identifier: GPL-2.0-only

//! Siemens industrial-module vendor MIB support (networking).
//!
//! Applies to Siemens AD modules (`1.3.6.1.4.1.4196`) and fills the `NETWORKING`
//! type, manufacturer, hostname, serial, firmware and model. The model is
//! resolved from the MLFB part number (a small known-model table, otherwise a
//! generic `Siemens module (PartNumber: …)` label). Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Siemens`; the `sysDescr`-based fallbacks and
//! the special `.0.0` sysObjectID case are not modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Siemens AD enterprise OID.
const AD: &str = "1.3.6.1.4.1.4196";
/// `snSwVersion` (`snGen.4.0`).
const SN_SW_VERSION: [u64; 16] = [1, 3, 6, 1, 4, 1, 4196, 1, 1, 8, 3, 100, 1, 8, 4, 0];
/// `snInfoSerialNr` (`snGen.6.0`).
const SN_INFO_SERIAL_NR: [u64; 16] = [1, 3, 6, 1, 4, 1, 4196, 1, 1, 8, 3, 100, 1, 8, 6, 0];
/// `snInfoMLFBNr` (`snGen.26.0`).
const SN_INFO_MLFB_NR: [u64; 16] = [1, 3, 6, 1, 4, 1, 4196, 1, 1, 8, 3, 100, 1, 8, 26, 0];
/// `snAsiLinkPnioDeviceName` (`iAsiLinkMib.2.21.2.0`).
const SN_ASI_LINK_PNIO_DEVICE_NAME: [u64; 16] =
    [1, 3, 6, 1, 4, 1, 4196, 1, 1, 8, 3, 100, 2, 21, 2, 0];
/// `moduleMLFB` (`siemens.6.3.2.1.1.2.0`).
const MODULE_MLFB: [u64; 14] = [1, 3, 6, 1, 4, 1, 4329, 6, 3, 2, 1, 1, 2, 0];
/// `moduleSerial` (`siemens.6.3.2.1.1.3.0`).
const MODULE_SERIAL: [u64; 14] = [1, 3, 6, 1, 4, 1, 4329, 6, 3, 2, 1, 1, 3, 0];
/// `moduleFirmware` (`siemens.6.3.2.1.1.5.0`).
const MODULE_FIRMWARE: [u64; 14] = [1, 3, 6, 1, 4, 1, 4329, 6, 3, 2, 1, 1, 5, 0];

/// Vendor MIB module for Siemens industrial modules.
#[derive(Debug, Default, Clone, Copy)]
pub struct SiemensMib;

#[async_trait]
impl MibSupport for SiemensMib {
    fn name(&self) -> &'static str {
        "siemens"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), AD)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Siemens".to_owned());
        }
        if device.info.model.is_none() {
            device.info.model = first_of(session, &[&SN_INFO_MLFB_NR, &MODULE_MLFB])
                .await?
                .map(|mlfb| model_for_mlfb(&mlfb));
        }

        let serial = first_of(session, &[&SN_INFO_SERIAL_NR, &MODULE_SERIAL])
            .await?
            .filter(|s| !s.contains("not set"));
        if device.info.firmware.is_none() {
            device.info.firmware = first_of(session, &[&SN_SW_VERSION, &MODULE_FIRMWARE]).await?;
        }
        if device.info.name.is_none() {
            device.info.name = match get_string(session, &SN_ASI_LINK_PNIO_DEVICE_NAME).await? {
                Some(name) => Some(name),
                None => serial.clone(),
            };
        }
        if device.info.serial.is_none() {
            device.info.serial = serial;
        }
        Ok(())
    }
}

/// Maps a known MLFB part number to its model name, or builds a generic label.
fn model_for_mlfb(mlfb: &str) -> String {
    match mlfb {
        "6GK1 411-2AB10" => "IE/AS-i LINK PN IO".to_owned(),
        "6GK7 343-1CX10-0XE0" => "CP 343-1 Lean".to_owned(),
        "6ES7 318-3EL01-0AB0" => "CPU319-3 PN/DP".to_owned(),
        other => format!("Siemens module (PartNumber: {other})"),
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
    use super::SiemensMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_siemens_ad() {
        assert!(SiemensMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.4196.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!SiemensMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.4197".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn maps_mlfb_to_model_and_reads_identity() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.4196.1.1.8.3.100.1.8.26.0 = STRING: \"6GK1 411-2AB10\"\n\
             .1.3.6.1.4.1.4196.1.1.8.3.100.1.8.6.0 = STRING: \"SVPL1234567\"\n\
             .1.3.6.1.4.1.4196.1.1.8.3.100.1.8.4.0 = STRING: \"V2.1.0\"\n\
             .1.3.6.1.4.1.4196.1.1.8.3.100.2.21.2.0 = STRING: \"asilink-1\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        SiemensMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Siemens"));
        assert_eq!(device.info.model.as_deref(), Some("IE/AS-i LINK PN IO"));
        assert_eq!(device.info.serial.as_deref(), Some("SVPL1234567"));
        assert_eq!(device.info.firmware.as_deref(), Some("V2.1.0"));
        assert_eq!(device.info.name.as_deref(), Some("asilink-1"));
    }

    #[tokio::test]
    async fn unknown_mlfb_gets_generic_label() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.4329.6.3.2.1.1.2.0 = STRING: \"6ZZ1 000-0AA00\"\n")
                .unwrap();
        let mut device = NetworkDevice::default();
        SiemensMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(
            device.info.model.as_deref(),
            Some("Siemens module (PartNumber: 6ZZ1 000-0AA00)")
        );
    }
}
