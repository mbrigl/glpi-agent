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

/// Collects the live USB devices (macOS) from `SPUSBDataType`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<UsbDevice> {
    crate::sys::output("system_profiler", &["-json", "SPUSBDataType"])
        .map(|json| parse_macos_usb(&json))
        .unwrap_or_default()
}

/// Collects the live USB devices (Windows) from `Win32_PnPEntity`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<UsbDevice> {
    crate::sys::powershell(
        "Get-CimInstance Win32_PnPEntity | Where-Object {$_.PNPDeviceID -like 'USB\\VID_*'} | \
         Select-Object Name,PNPDeviceID | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_usb(&json))
    .unwrap_or_default()
}

/// Collects the live USB devices (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<UsbDevice> {
    Vec::new()
}

/// Parses a `Win32_PnPEntity` `ConvertTo-Json` result into USB devices, reading
/// the `VID_`/`PID_` fields of each `PNPDeviceID`.
#[must_use]
pub fn parse_win_usb(json: &str) -> Vec<UsbDevice> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            let id = crate::jsonutil::str_field(item, "PNPDeviceID")?;
            Some(UsbDevice {
                vendorid: hex4_after(&id, "VID_")?,
                productid: hex4_after(&id, "PID_")?,
                name: crate::jsonutil::str_field(item, "Name"),
            })
        })
        .collect()
}

/// Parses `system_profiler -json SPUSBDataType` (macOS) into USB devices,
/// recursing through the hub tree (`_items`).
#[must_use]
pub fn parse_macos_usb(json: &str) -> Vec<UsbDevice> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut devices = Vec::new();
    for node in value
        .get("SPUSBDataType")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_macos_usb(node, &mut devices);
    }
    devices
}

/// Recursively collects USB devices from a `SPUSBDataType` node and its
/// `_items` children (a device has both a `vendor_id` and a `product_id`).
fn collect_macos_usb(node: &serde_json::Value, out: &mut Vec<UsbDevice>) {
    if let (Some(vendor), Some(product)) = (
        crate::jsonutil::str_field(node, "vendor_id")
            .as_deref()
            .and_then(hex4),
        crate::jsonutil::str_field(node, "product_id")
            .as_deref()
            .and_then(hex4),
    ) {
        out.push(UsbDevice {
            vendorid: vendor,
            productid: product,
            name: crate::jsonutil::str_field(node, "_name"),
        });
    }
    for child in node
        .get("_items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_macos_usb(child, out);
    }
}

/// Reads the four hex digits following `marker` (e.g. `"VID_"`), lower-cased.
fn hex4_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    hex4(&text[start..])
}

/// Reads four leading hex digits (after an optional `0x`), lower-cased.
fn hex4(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.strip_prefix("0x").unwrap_or(value);
    let hex: String = value.chars().take(4).collect();
    (hex.len() == 4 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hex.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::parse_lsusb;

    #[test]
    fn parses_windows_pnp_usb_json() {
        use super::parse_win_usb;
        let json = r#"[{"Name":"USB Composite Device","PNPDeviceID":"USB\\VID_8087&PID_0024\\5&abc"},
            {"Name":"No IDs","PNPDeviceID":"USB\\NOIDS"}]"#;
        let devices = parse_win_usb(json);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].vendorid, "8087");
        assert_eq!(devices[0].productid, "0024");
        assert_eq!(devices[0].name.as_deref(), Some("USB Composite Device"));
    }

    #[test]
    fn parses_macos_usb_tree() {
        use super::parse_macos_usb;
        let json = r#"{"SPUSBDataType":[{"_name":"USB3.0 Hub","_items":[
            {"_name":"Keyboard","vendor_id":"0x05ac (Apple Inc.)","product_id":"0x0250"}]}]}"#;
        let devices = parse_macos_usb(json);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].vendorid, "05ac");
        assert_eq!(devices[0].productid, "0250");
        assert_eq!(devices[0].name.as_deref(), Some("Keyboard"));
    }

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
