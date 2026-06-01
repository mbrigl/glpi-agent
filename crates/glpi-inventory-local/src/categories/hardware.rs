// SPDX-License-Identifier: GPL-2.0-only

//! Hardware / BIOS inventory category (Linux `dmidecode`).
//!
//! Extracts the device identity from the SMBIOS structures: BIOS (type 0),
//! System (type 1), Base Board (type 2) and Chassis (type 3). The BIOS-level
//! fields populate [`Bios`] (vendor/version/date plus the system and
//! motherboard manufacturer/model/serial and asset tag, matching the GLPI
//! `bios` keys); the system UUID and the hostname populate [`Hardware`].

use serde::Serialize;

use super::dmi::{self, clean};

/// The GLPI `bios` section: BIOS, system and motherboard identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Bios {
    /// BIOS release date (`bdate`).
    #[serde(rename = "bdate", skip_serializing_if = "Option::is_none")]
    pub bios_date: Option<String>,
    /// BIOS vendor (`bmanufacturer`).
    #[serde(rename = "bmanufacturer", skip_serializing_if = "Option::is_none")]
    pub bios_manufacturer: Option<String>,
    /// BIOS version (`bversion`).
    #[serde(rename = "bversion", skip_serializing_if = "Option::is_none")]
    pub bios_version: Option<String>,
    /// System manufacturer (`smanufacturer`).
    #[serde(rename = "smanufacturer", skip_serializing_if = "Option::is_none")]
    pub system_manufacturer: Option<String>,
    /// System model / product name (`smodel`).
    #[serde(rename = "smodel", skip_serializing_if = "Option::is_none")]
    pub system_model: Option<String>,
    /// System serial number (`ssn`).
    #[serde(rename = "ssn", skip_serializing_if = "Option::is_none")]
    pub system_serial: Option<String>,
    /// Motherboard manufacturer (`mmanufacturer`).
    #[serde(rename = "mmanufacturer", skip_serializing_if = "Option::is_none")]
    pub board_manufacturer: Option<String>,
    /// Motherboard model (`mmodel`).
    #[serde(rename = "mmodel", skip_serializing_if = "Option::is_none")]
    pub board_model: Option<String>,
    /// Motherboard serial (`msn`).
    #[serde(rename = "msn", skip_serializing_if = "Option::is_none")]
    pub board_serial: Option<String>,
    /// Chassis asset tag (`assettag`).
    #[serde(rename = "assettag", skip_serializing_if = "Option::is_none")]
    pub asset_tag: Option<String>,
}

impl Bios {
    /// `true` if no field was populated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Bios::default()
    }
}

/// The GLPI `hardware` section: device-level identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Hardware {
    /// Host name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// SMBIOS system UUID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

impl Hardware {
    /// `true` if no field was populated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Hardware::default()
    }
}

/// Parses `dmidecode` output into the BIOS and hardware identity.
#[must_use]
pub fn parse_dmidecode_hardware(text: &str) -> (Bios, Hardware) {
    let mut bios = Bios::default();
    let mut hardware = Hardware::default();

    for block in dmi::parse_blocks(text) {
        match block.name.as_str() {
            "BIOS Information" => {
                bios.bios_date = clean(block.get("Release Date"));
                bios.bios_manufacturer = clean(block.get("Vendor"));
                bios.bios_version = clean(block.get("Version"));
            }
            "System Information" => {
                bios.system_manufacturer = clean(block.get("Manufacturer"));
                bios.system_model = clean(block.get("Product Name"));
                bios.system_serial = clean(block.get("Serial Number"));
                hardware.uuid = clean(block.get("UUID"));
            }
            "Base Board Information" => {
                bios.board_manufacturer = clean(block.get("Manufacturer"));
                bios.board_model = clean(block.get("Product Name"));
                bios.board_serial = clean(block.get("Serial Number"));
            }
            "Chassis Information" => {
                bios.asset_tag = clean(block.get("Asset Tag"));
            }
            _ => {}
        }
    }
    (bios, hardware)
}

/// Collects the live BIOS and hardware identity (Linux).
///
/// `dmidecode` requires root, so the BIOS/system fields may be empty in an
/// unprivileged environment; the hostname is always set when available.
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> (Option<Bios>, Option<Hardware>) {
    let text = match std::process::Command::new("dmidecode").output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        _ => String::new(),
    };
    let (bios, mut hardware) = parse_dmidecode_hardware(&text);
    hardware.name = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    let bios = (!bios.is_empty()).then_some(bios);
    let hardware = (!hardware.is_empty()).then_some(hardware);
    (bios, hardware)
}

/// Collects the live BIOS and hardware identity (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> (Option<Bios>, Option<Hardware>) {
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::parse_dmidecode_hardware;

    const DMIDECODE: &str = "\
Handle 0x0000, DMI type 0, 26 bytes
BIOS Information
\tVendor: American Megatrends Inc.
\tVersion: 1.2.0
\tRelease Date: 03/15/2023

Handle 0x0001, DMI type 1, 27 bytes
System Information
\tManufacturer: Dell Inc.
\tProduct Name: OptiPlex 7090
\tSerial Number: ABC1234
\tUUID: 4c4c4544-0042-1234-8000-abcdef123456

Handle 0x0002, DMI type 2, 15 bytes
Base Board Information
\tManufacturer: Dell Inc.
\tProduct Name: 0ABCD1
\tSerial Number: /ABC1234/

Handle 0x0003, DMI type 3, 22 bytes
Chassis Information
\tManufacturer: Dell Inc.
\tAsset Tag: To Be Filled By O.E.M.
";

    #[test]
    fn extracts_bios_system_and_board() {
        let (bios, hardware) = parse_dmidecode_hardware(DMIDECODE);
        assert_eq!(
            bios.bios_manufacturer.as_deref(),
            Some("American Megatrends Inc.")
        );
        assert_eq!(bios.bios_version.as_deref(), Some("1.2.0"));
        assert_eq!(bios.bios_date.as_deref(), Some("03/15/2023"));
        assert_eq!(bios.system_manufacturer.as_deref(), Some("Dell Inc."));
        assert_eq!(bios.system_model.as_deref(), Some("OptiPlex 7090"));
        assert_eq!(bios.system_serial.as_deref(), Some("ABC1234"));
        assert_eq!(bios.board_model.as_deref(), Some("0ABCD1"));
        // Placeholder asset tag is dropped.
        assert_eq!(bios.asset_tag, None);

        assert_eq!(
            hardware.uuid.as_deref(),
            Some("4c4c4544-0042-1234-8000-abcdef123456")
        );
    }

    #[test]
    fn empty_input_is_empty() {
        let (bios, hardware) = parse_dmidecode_hardware("");
        assert!(bios.is_empty());
        assert!(hardware.is_empty());
    }
}
