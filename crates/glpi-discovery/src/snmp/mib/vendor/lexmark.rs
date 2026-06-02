// SPDX-License-Identifier: GPL-2.0-only

//! Lexmark printer vendor MIB support.
//!
//! Applies to devices under the Lexmark enterprise tree (`1.3.6.1.4.1.641`) and
//! fills the model, firmware revision and serial, each from an ordered list of
//! candidate OIDs (the MPS inventory tree, the PVT general-info tree, the
//! standard Printer-MIB and HOST-RESOURCES-MIB). Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Lexmark`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// Lexmark enterprise OID (`LEXMARK-ROOT-MIB`).
const LEXMARK: &str = "1.3.6.1.4.1.641";

// LEXMARK-PVT-MIB general-info entry. The upstream constants concatenate
// `printer` (`641.2`) with the string `"1.2.1"` *without* a separating dot, so
// the effective base is `641.21.2.1`; we replicate that exact numeric OID.
/// `prtgenPrinterName` (`prtgenInfoEntry.2.1`).
const PRTGEN_PRINTER_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 641, 21, 2, 1, 2, 1];
/// `prtgenCodeRevision` (`prtgenInfoEntry.4.1`).
const PRTGEN_CODE_REVISION: [u64; 12] = [1, 3, 6, 1, 4, 1, 641, 21, 2, 1, 4, 1];
/// `prtgenSerialNo` (`prtgenInfoEntry.6.1`).
const PRTGEN_SERIAL_NO: [u64; 12] = [1, 3, 6, 1, 4, 1, 641, 21, 2, 1, 6, 1];

// LEXMARK-MPS-MIB.
/// `deviceModel` (`mps.device.3.1.4.1`, OID name extrapolated upstream).
const DEVICE_MODEL: [u64; 13] = [1, 3, 6, 1, 4, 1, 641, 6, 2, 3, 1, 4, 1];
/// `deviceSerial` (`mps.device.3.1.5.1`, OID name extrapolated upstream).
const DEVICE_SERIAL: [u64; 13] = [1, 3, 6, 1, 4, 1, 641, 6, 2, 3, 1, 5, 1];
/// `swInventoryRevision` (`mps.inventory.3.1.7.1.1`).
const SW_INVENTORY_REVISION: [u64; 14] = [1, 3, 6, 1, 4, 1, 641, 6, 3, 3, 1, 7, 1, 1];

/// `prtGeneralSerialNumber` (Printer-MIB `43.5.1.1.17.1`).
const PRT_GENERAL_SERIAL_NUMBER: [u64; 12] = [1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 17, 1];
/// `hrDeviceDescr` (HOST-RESOURCES-MIB `25.3.2.1.3.1`).
const HR_DEVICE_DESCR: [u64; 12] = [1, 3, 6, 1, 2, 1, 25, 3, 2, 1, 3, 1];

/// Vendor MIB module for Lexmark printers.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexmarkMib;

#[async_trait]
impl MibSupport for LexmarkMib {
    fn name(&self) -> &'static str {
        "lexmark-printer"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), LEXMARK)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.model.is_none() {
            device.info.model = self.model(session).await?;
        }
        if device.info.firmware.is_none() {
            device.info.firmware =
                first_of(session, &[&SW_INVENTORY_REVISION, &PRTGEN_CODE_REVISION]).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = first_of(
                session,
                &[
                    &PRT_GENERAL_SERIAL_NUMBER,
                    &DEVICE_SERIAL,
                    &PRTGEN_SERIAL_NO,
                ],
            )
            .await?;
        }
        Ok(())
    }
}

impl LexmarkMib {
    /// Resolves the model: the MPS/PVT names first, then the `hrDeviceDescr`
    /// fallback (taking the leading `Lexmark <word>`). The "Lexmark " prefix is
    /// stripped from whichever source matched.
    async fn model(&self, session: &mut dyn SnmpQuery) -> Result<Option<String>> {
        let model = match first_of(session, &[&DEVICE_MODEL, &PRTGEN_PRINTER_NAME]).await? {
            Some(model) => Some(model),
            None => get_string(session, &HR_DEVICE_DESCR)
                .await?
                .and_then(|descr| lexmark_from_descr(&descr)),
        };
        Ok(model.map(|m| {
            m.strip_prefix("Lexmark ")
                .or_else(|| m.strip_prefix("LEXMARK "))
                .unwrap_or(&m)
                .to_owned()
        }))
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

/// Extracts a leading `Lexmark <model>` token from a `hrDeviceDescr` string.
fn lexmark_from_descr(descr: &str) -> Option<String> {
    let rest = descr.strip_prefix("Lexmark ")?;
    let word = rest.split_whitespace().next()?;
    Some(format!("Lexmark {word}"))
}

#[cfg(test)]
mod tests {
    use super::LexmarkMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_lexmark_enterprise() {
        assert!(LexmarkMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.641.2.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!LexmarkMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.6411".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn prefers_mps_oids_and_strips_prefix() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.641.6.2.3.1.4.1 = STRING: \"Lexmark MX622\"\n\
             .1.3.6.1.4.1.641.6.3.3.1.7.1.1 = STRING: \"LW80.PR2.P231\"\n\
             .1.3.6.1.2.1.43.5.1.1.17.1 = STRING: \"7654321\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        LexmarkMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("MX622"));
        assert_eq!(device.info.firmware.as_deref(), Some("LW80.PR2.P231"));
        assert_eq!(device.info.serial.as_deref(), Some("7654321"));
    }

    #[tokio::test]
    async fn falls_back_to_host_resources_description() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.2.1.25.3.2.1.3.1 = STRING: \"Lexmark CX417 laser printer\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        LexmarkMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("CX417"));
    }
}
