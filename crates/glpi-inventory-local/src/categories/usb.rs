// SPDX-License-Identifier: GPL-2.0-only

//! USB device inventory category (Linux `lsusb`).
//!
//! Parses `lsusb` lines of the form `Bus … Device …: ID vvvv:pppp NAME` into
//! [`UsbDevice`] records (vendor id, product id, name) for the GLPI
//! `usbdevices` section. The parser is pure and unit-tested; the live collector
//! runs `lsusb`.

use serde::Serialize;

/// A USB device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UsbDevice {
    /// Vendor id (4 hex digits).
    pub vendorid: String,
    /// Product id (4 hex digits).
    pub productid: String,
    /// Device description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Parses `lsusb` output into USB devices.
#[must_use]
pub fn parse_lsusb(text: &str) -> Vec<UsbDevice> {
    text.lines()
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let id_pos = tokens.iter().position(|t| *t == "ID")?;
            let (vendorid, productid) = tokens.get(id_pos + 1)?.split_once(':')?;
            if vendorid.is_empty() || productid.is_empty() {
                return None;
            }
            let name = tokens.get(id_pos + 2..).map(|rest| rest.join(" "));
            Some(UsbDevice {
                vendorid: vendorid.to_owned(),
                productid: productid.to_owned(),
                name: name.filter(|n| !n.is_empty()),
            })
        })
        .collect()
}

/// Collects the live USB devices via `lsusb` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<UsbDevice> {
    match std::process::Command::new("lsusb").output() {
        Ok(output) if output.status.success() => {
            parse_lsusb(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live USB devices (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<UsbDevice> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::parse_lsusb;

    const LSUSB: &str = "\
Bus 001 Device 002: ID 8087:0024 Intel Corp. Integrated Rate Matching Hub
Bus 002 Device 001: ID 1d6b:0003 Linux Foundation 3.0 root hub
Bus 001 Device 005: ID 046d:c52b
";

    #[test]
    fn parses_devices_with_and_without_names() {
        let devices = parse_lsusb(LSUSB);
        assert_eq!(devices.len(), 3);

        assert_eq!(devices[0].vendorid, "8087");
        assert_eq!(devices[0].productid, "0024");
        assert_eq!(
            devices[0].name.as_deref(),
            Some("Intel Corp. Integrated Rate Matching Hub")
        );

        assert_eq!(
            devices[1].name.as_deref(),
            Some("Linux Foundation 3.0 root hub")
        );

        // No trailing description.
        assert_eq!(devices[2].vendorid, "046d");
        assert_eq!(devices[2].name, None);
    }

    #[test]
    fn empty_input_yields_no_devices() {
        assert!(parse_lsusb("").is_empty());
    }
}
