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

    /// Fills any field still unset from `fallback`.
    ///
    /// Used to back-fill the `/sys/class/dmi/id/` values behind `dmidecode`:
    /// `dmidecode` wins where it produced a value, sysfs covers the rest.
    fn fill_from(&mut self, fallback: Bios) {
        self.bios_date = self.bios_date.take().or(fallback.bios_date);
        self.bios_manufacturer = self.bios_manufacturer.take().or(fallback.bios_manufacturer);
        self.bios_version = self.bios_version.take().or(fallback.bios_version);
        self.system_manufacturer = self
            .system_manufacturer
            .take()
            .or(fallback.system_manufacturer);
        self.system_model = self.system_model.take().or(fallback.system_model);
        self.system_serial = self.system_serial.take().or(fallback.system_serial);
        self.board_manufacturer = self
            .board_manufacturer
            .take()
            .or(fallback.board_manufacturer);
        self.board_model = self.board_model.take().or(fallback.board_model);
        self.board_serial = self.board_serial.take().or(fallback.board_serial);
        self.asset_tag = self.asset_tag.take().or(fallback.asset_tag);
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
    /// Virtualization system ("Physical", "VMware", "Docker", …).
    #[serde(rename = "vmsystem", skip_serializing_if = "Option::is_none")]
    pub vm_system: Option<String>,
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

/// Builds the BIOS and hardware identity from `/sys/class/dmi/id/` entries.
///
/// This is the fallback for when `dmidecode` is unavailable — it is not
/// installed everywhere, and reading SMBIOS via `/dev/mem` requires root.
/// `read` returns the trimmed contents of the named DMI id file (e.g.
/// `"product_name"`). Most entries are world-readable, but the serial-number
/// and UUID files (`product_serial`, `board_serial`, `product_uuid`) are
/// root-only, so those stay `None` for an unprivileged agent. Values pass
/// through [`clean`] to drop the SMBIOS placeholders.
#[must_use]
pub fn parse_dmi_sysfs<F>(read: F) -> (Bios, Hardware)
where
    F: Fn(&str) -> Option<String>,
{
    let field = |name: &str| clean(read(name).as_deref());
    let bios = Bios {
        bios_date: field("bios_date"),
        bios_manufacturer: field("bios_vendor"),
        bios_version: field("bios_version"),
        system_manufacturer: field("sys_vendor"),
        system_model: field("product_name"),
        system_serial: field("product_serial"),
        board_manufacturer: field("board_vendor"),
        board_model: field("board_name"),
        board_serial: field("board_serial"),
        asset_tag: field("chassis_asset_tag"),
    };
    let hardware = Hardware {
        name: None,
        uuid: field("product_uuid"),
        vm_system: None,
    };
    (bios, hardware)
}

/// Collects the live BIOS and hardware identity (Linux).
///
/// `dmidecode` is queried first; any field it leaves empty (it is not always
/// installed, and reading SMBIOS needs root) is back-filled from
/// `/sys/class/dmi/id/`. The hostname is always set when available; the
/// serial-number and UUID fields still require root on either path.
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> (Option<Bios>, Option<Hardware>) {
    let text = match std::process::Command::new("dmidecode").output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        _ => String::new(),
    };
    let (mut bios, mut hardware) = parse_dmidecode_hardware(&text);

    // Back-fill from sysfs whatever dmidecode could not supply.
    let (sysfs_bios, sysfs_hardware) = parse_dmi_sysfs(read_dmi_id);
    bios.fill_from(sysfs_bios);
    hardware.uuid = hardware.uuid.take().or(sysfs_hardware.uuid);

    hardware.name = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    hardware.vm_system = detect_vm_system();

    let bios = (!bios.is_empty()).then_some(bios);
    let hardware = (!hardware.is_empty()).then_some(hardware);
    (bios, hardware)
}

/// Reads a `/sys/class/dmi/id/` entry, trimmed, or `None` if absent/unreadable.
#[cfg(target_os = "linux")]
fn read_dmi_id(name: &str) -> Option<String> {
    std::fs::read_to_string(format!("/sys/class/dmi/id/{name}"))
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Collects the live BIOS and hardware identity (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> (Option<Bios>, Option<Hardware>) {
    (None, None)
}

/// Detects the virtualization system via `systemd-detect-virt`.
#[cfg(target_os = "linux")]
fn detect_vm_system() -> Option<String> {
    let output = std::process::Command::new("systemd-detect-virt")
        .output()
        .ok()?;
    let raw = String::from_utf8_lossy(&output.stdout);
    Some(map_vm_system(raw.trim()))
}

/// Maps a `systemd-detect-virt` token to a GLPI-style virtualization name.
fn map_vm_system(token: &str) -> String {
    match token {
        "" | "none" => "Physical",
        "kvm" | "qemu" => "QEMU",
        "vmware" => "VMware",
        "oracle" => "VirtualBox",
        "microsoft" => "Hyper-V",
        "xen" => "Xen",
        "docker" => "Docker",
        "podman" => "Podman",
        "lxc" | "lxc-libvirt" => "LXC",
        "openvz" => "OpenVZ",
        "wsl" => "WSL",
        other => return capitalize(other),
    }
    .to_owned()
}

/// Capitalizes the first letter of an unrecognized token.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{parse_dmi_sysfs, parse_dmidecode_hardware};

    #[test]
    fn reads_identity_from_sysfs() {
        // Models an unprivileged read: world-readable files resolve, the
        // root-only serial/UUID files return None.
        let (bios, hardware) = parse_dmi_sysfs(|name| match name {
            "sys_vendor" => Some("Dell Inc.".to_owned()),
            "product_name" => Some("Precision 3460".to_owned()),
            "board_vendor" => Some("Dell Inc.".to_owned()),
            "board_name" => Some("08PFGW".to_owned()),
            "bios_vendor" => Some("Dell Inc.".to_owned()),
            "bios_version" => Some("3.23.1".to_owned()),
            "chassis_asset_tag" => Some("To Be Filled By O.E.M.".to_owned()),
            // Root-only files unreadable for an unprivileged agent.
            "product_serial" | "board_serial" | "product_uuid" => None,
            _ => None,
        });
        assert_eq!(bios.system_manufacturer.as_deref(), Some("Dell Inc."));
        assert_eq!(bios.system_model.as_deref(), Some("Precision 3460"));
        assert_eq!(bios.board_model.as_deref(), Some("08PFGW"));
        assert_eq!(bios.bios_version.as_deref(), Some("3.23.1"));
        // Placeholder asset tag dropped, root-only fields stay empty.
        assert_eq!(bios.asset_tag, None);
        assert_eq!(bios.system_serial, None);
        assert_eq!(hardware.uuid, None);
    }

    #[test]
    fn dmidecode_values_win_over_sysfs_backfill() {
        let mut bios = super::Bios {
            system_model: Some("From dmidecode".to_owned()),
            ..super::Bios::default()
        };
        let (sysfs, _) = parse_dmi_sysfs(|name| match name {
            "product_name" => Some("From sysfs".to_owned()),
            "sys_vendor" => Some("ACME".to_owned()),
            _ => None,
        });
        bios.fill_from(sysfs);
        // dmidecode's value is kept; the unset manufacturer is back-filled.
        assert_eq!(bios.system_model.as_deref(), Some("From dmidecode"));
        assert_eq!(bios.system_manufacturer.as_deref(), Some("ACME"));
    }

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

    #[test]
    fn maps_vm_system_tokens() {
        use super::map_vm_system;
        assert_eq!(map_vm_system("none"), "Physical");
        assert_eq!(map_vm_system(""), "Physical");
        assert_eq!(map_vm_system("docker"), "Docker");
        assert_eq!(map_vm_system("kvm"), "QEMU");
        assert_eq!(map_vm_system("vmware"), "VMware");
        // Unknown tokens are capitalized.
        assert_eq!(map_vm_system("acme"), "Acme");
    }
}
