// SPDX-License-Identifier: GPL-2.0-only

//! HP Citizen (HP storage) vendor MIB support.
//!
//! Applies to HP storage devices under `hpCitizen` (`1.3.6.1.4.1.11.10`) and
//! reads identity from the SEMI-MIB HTTP-management Net-Citizen group. Sets the
//! `STORAGE` type, manufacturer, firmware, serial and model, and records the
//! hardware and ROM versions as firmware entries. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::HPCitizen`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `hpCitizen` (`hp.10`).
const HP_CITIZEN: &str = "1.3.6.1.4.1.11.10";
/// `hpHttpMgManufacturer` (`hpHttpMgNetCitizen.4.0`).
const HP_MANUFACTURER: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 4];
/// `hpHttpMgProduct` (`hpHttpMgNetCitizen.5.0`).
const HP_PRODUCT: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 5];
/// `hpHttpMgVersion` (`hpHttpMgNetCitizen.6.0`).
const HP_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 6];
/// `hpHttpMgHWVersion` (`hpHttpMgNetCitizen.7.0`).
const HP_HW_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 7];
/// `hpHttpMgROMVersion` (`hpHttpMgNetCitizen.8.0`).
const HP_ROM_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 8];
/// `hpHttpMgSerialNumber` (`hpHttpMgNetCitizen.9.0`).
const HP_SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 11, 2, 36, 1, 1, 2, 9];

/// Vendor MIB module for HP storage (Citizen) devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct HpCitizenMib;

#[async_trait]
impl MibSupport for HpCitizenMib {
    fn name(&self) -> &'static str {
        "hp-citizen"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), HP_CITIZEN)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // Index `.0` instances.
        let manufacturer = scalar(session, &HP_MANUFACTURER)
            .await?
            .filter(|m| m != "HP")
            .unwrap_or_else(|| "Hewlett-Packard".to_owned());

        if device.info.r#type.is_none() {
            device.info.r#type = Some("STORAGE".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some(manufacturer.clone());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = scalar(session, &HP_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = scalar(session, &HP_SERIAL_NUMBER).await?;
        }
        let model = scalar(session, &HP_PRODUCT).await?;
        if device.info.model.is_none() {
            device.info.model.clone_from(&model);
        }

        let Some(model) = model else {
            return Ok(());
        };
        if let Some(version) = scalar(session, &HP_HW_VERSION).await? {
            device.add_firmware(make(
                &format!("{model} HW"),
                &format!("{model} HW version"),
                version,
                &manufacturer,
            ));
        }
        if let Some(version) = scalar(session, &HP_ROM_VERSION)
            .await?
            .filter(|v| !v.eq_ignore_ascii_case("null"))
        {
            device.add_firmware(make(
                &format!("{model} Rom"),
                &format!("{model} Rom version"),
                version,
                &manufacturer,
            ));
        }
        Ok(())
    }
}

/// Reads the `.0` instance of `oid` as a non-empty string.
async fn scalar(session: &mut dyn SnmpQuery, oid: &[u64]) -> Result<Option<String>> {
    let full: Vec<u64> = oid.iter().copied().chain(std::iter::once(0)).collect();
    get_string(session, &full).await
}

/// Builds an HP hardware firmware entry.
fn make(name: &str, description: &str, version: String, manufacturer: &str) -> Firmware {
    Firmware {
        name: Some(name.to_owned()),
        description: Some(description.to_owned()),
        r#type: Some("hardware".to_owned()),
        version: Some(version),
        manufacturer: Some(manufacturer.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::HpCitizenMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_hp_citizen() {
        assert!(HpCitizenMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.11.10.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!HpCitizenMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.11.2".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_storage_identity_and_firmwares() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.11.2.36.1.1.2.4.0 = STRING: \"HP\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.5.0 = STRING: \"StorageWorks 1606\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.6.0 = STRING: \"V7.1\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.7.0 = STRING: \"A\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.8.0 = STRING: \"null\"\n\
             .1.3.6.1.4.1.11.2.36.1.1.2.9.0 = STRING: \"HPSN0001\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        HpCitizenMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("STORAGE"));
        // Manufacturer "HP" is normalised to "Hewlett-Packard".
        assert_eq!(device.info.manufacturer.as_deref(), Some("Hewlett-Packard"));
        assert_eq!(device.info.firmware.as_deref(), Some("V7.1"));
        assert_eq!(device.info.serial.as_deref(), Some("HPSN0001"));
        assert_eq!(device.info.model.as_deref(), Some("StorageWorks 1606"));
        // ROM "null" is skipped, only the HW firmware is recorded.
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("StorageWorks 1606 HW")
        );
    }
}
