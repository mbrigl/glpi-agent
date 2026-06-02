// SPDX-License-Identifier: GPL-2.0-only

//! Monitor inventory category (Linux EDID via `/sys/class/drm`).
//!
//! Parses the 128-byte EDID block exposed at `/sys/class/drm/*/edid` into a
//! [`Monitor`]: the manufacturer's 3-letter PNP id, the product code, the
//! manufacture year, and — from the descriptor blocks — the monitor name and
//! serial. (The PNP id → vendor-name lookup via `edid.ids` is a later
//! refinement.) The byte parser is pure and unit-tested; the live collector
//! reads the sysfs EDID files.

use serde::Serialize;

/// A monitor decoded from its EDID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Monitor {
    /// Manufacturer PNP id (3 letters, e.g. "DEL").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Monitor name (from the descriptor block), the GLPI caption.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Serial number (descriptor string, else the numeric serial).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// Product code (hex).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Manufacture year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
}

const EDID_HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
/// Descriptor tag for the monitor name.
const TAG_NAME: u8 = 0xFC;
/// Descriptor tag for the monitor serial.
const TAG_SERIAL: u8 = 0xFF;
/// The four 18-byte descriptor block offsets.
const DESCRIPTORS: [usize; 4] = [54, 72, 90, 108];

/// Parses a 128-byte EDID block.
///
/// Returns `None` if the data is too short or the fixed EDID header is missing.
#[must_use]
pub fn parse_edid(edid: &[u8]) -> Option<Monitor> {
    if edid.len() < 128 || edid[0..8] != EDID_HEADER {
        return None;
    }

    let descriptor_text = |tag: u8| {
        DESCRIPTORS
            .iter()
            .find_map(|&off| descriptor(&edid[off..off + 18], tag))
    };

    let numeric_serial = u32::from_le_bytes([edid[12], edid[13], edid[14], edid[15]]);
    let serial = descriptor_text(TAG_SERIAL)
        .or_else(|| (numeric_serial != 0).then(|| numeric_serial.to_string()));

    let year = (edid[17] != 0).then(|| u32::from(edid[17]) + 1990);

    Some(Monitor {
        manufacturer: manufacturer(edid),
        caption: descriptor_text(TAG_NAME),
        serial,
        model: Some(format!("{:04X}", u16::from_le_bytes([edid[10], edid[11]]))),
        year,
    })
}

/// Collects monitors by reading each `/sys/class/drm/*/edid` file (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Monitor> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| std::fs::read(entry.path().join("edid")).ok())
        .filter(|edid| !edid.is_empty())
        .filter_map(|edid| parse_edid(&edid))
        .collect()
}

/// Collects monitors (macOS) by extracting the `IODisplayEDID` blobs from
/// `ioreg` and decoding each.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Monitor> {
    crate::sys::output("ioreg", &["-l", "-w0"])
        .map(|text| parse_ioreg_edid(&text))
        .unwrap_or_default()
}

/// Collects monitors (Windows) by reading each display's EDID from the
/// registry (`…\DISPLAY\…\Device Parameters\EDID`).
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Monitor> {
    crate::sys::powershell(
        "Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Enum\\DISPLAY' -Recurse \
         -ErrorAction SilentlyContinue | Where-Object {$_.Property -contains 'EDID'} | \
         ForEach-Object {[pscustomobject]@{EDID=(Get-ItemProperty -Path $_.PSPath -Name EDID).EDID}} | \
         ConvertTo-Json -Compress -Depth 4",
    )
    .map(|json| parse_win_monitors(&json))
    .unwrap_or_default()
}

/// Collects monitors (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Monitor> {
    Vec::new()
}

/// Parses the Windows registry EDID dump (`[{ "EDID": [byte, …] }, …]`) into
/// monitors by decoding each EDID byte array.
#[must_use]
pub fn parse_win_monitors(json: &str) -> Vec<Monitor> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            let bytes: Vec<u8> = item
                .get("EDID")
                .and_then(serde_json::Value::as_array)?
                .iter()
                .filter_map(|n| n.as_u64().map(|b| b as u8))
                .collect();
            parse_edid(&bytes)
        })
        .collect()
}

/// Parses `ioreg -l` output (macOS), decoding each `"IODisplayEDID" = <hex>`
/// blob into a monitor.
#[must_use]
pub fn parse_ioreg_edid(text: &str) -> Vec<Monitor> {
    text.lines()
        .filter(|line| line.contains("IODisplayEDID"))
        .filter_map(|line| {
            let start = line.find('<')?;
            let end = line[start..].find('>')? + start;
            let bytes = decode_hex(&line[start + 1..end])?;
            parse_edid(&bytes)
        })
        .collect()
}

/// Decodes a whitespace-tolerant hex string into bytes.
fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    let hex: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

/// Decodes the 3-letter PNP manufacturer id from bytes 8–9.
fn manufacturer(edid: &[u8]) -> Option<String> {
    let id = u16::from_be_bytes([edid[8], edid[9]]);
    let letters = [(id >> 10) & 0x1f, (id >> 5) & 0x1f, id & 0x1f];
    if letters.contains(&0) {
        return None;
    }
    Some(
        letters
            .iter()
            .map(|&c| char::from(b'A' - 1 + c as u8))
            .collect(),
    )
}

/// Returns the ASCII text of an 18-byte descriptor block if it carries `tag`.
fn descriptor(block: &[u8], tag: u8) -> Option<String> {
    if block[0..3] != [0, 0, 0] || block[3] != tag || block[4] != 0 {
        return None;
    }
    let text: String = block[5..18]
        .iter()
        .take_while(|&&c| c != 0x0A)
        .map(|&c| char::from(c))
        .collect();
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::{parse_edid, EDID_HEADER};

    /// Builds a 128-byte EDID for manufacturer "DEL", with a monitor-name
    /// descriptor and a numeric serial.
    fn sample_edid() -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[0..8].copy_from_slice(&EDID_HEADER);
        // "DEL" -> 0x10AC, stored big-endian.
        e[8] = 0x10;
        e[9] = 0xAC;
        // Product code 0xA0F0, little-endian.
        e[10] = 0xF0;
        e[11] = 0xA0;
        // Serial number 1, little-endian.
        e[12] = 0x01;
        // Week 10, year 2014 (1990 + 24).
        e[16] = 10;
        e[17] = 24;
        // Monitor-name descriptor at offset 54: tag 0xFC, "DELL U2412".
        let d = 54;
        e[d + 3] = 0xFC;
        let name = b"DELL U2412";
        e[d + 5..d + 5 + name.len()].copy_from_slice(name);
        e[d + 5 + name.len()] = 0x0A; // line-feed terminator
        e
    }

    #[test]
    fn parses_manufacturer_name_and_serial() {
        let monitor = parse_edid(&sample_edid()).unwrap();
        assert_eq!(monitor.manufacturer.as_deref(), Some("DEL"));
        assert_eq!(monitor.caption.as_deref(), Some("DELL U2412"));
        assert_eq!(monitor.model.as_deref(), Some("A0F0"));
        assert_eq!(monitor.year, Some(2014));
        // No serial descriptor, so the numeric serial is used.
        assert_eq!(monitor.serial.as_deref(), Some("1"));
    }

    #[test]
    fn rejects_bad_header_and_short_data() {
        assert_eq!(parse_edid(&[0u8; 128]), None);
        assert_eq!(parse_edid(&[0u8; 10]), None);
    }

    #[test]
    fn parses_windows_registry_edid() {
        use super::parse_win_monitors;
        let edid = sample_edid();
        // Model the `[{ "EDID": [..] }]` shape PowerShell produces.
        let json = serde_json::to_string(&serde_json::json!([{ "EDID": edid }])).unwrap();
        let monitors = parse_win_monitors(&json);
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].manufacturer.as_deref(), Some("DEL"));
        assert_eq!(monitors[0].caption.as_deref(), Some("DELL U2412"));
    }

    #[test]
    fn parses_macos_ioreg_edid() {
        use super::parse_ioreg_edid;
        let hex: String = sample_edid().iter().map(|b| format!("{b:02x}")).collect();
        let text = format!("    | |   \"IODisplayEDID\" = <{hex}>\n");
        let monitors = parse_ioreg_edid(&text);
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].caption.as_deref(), Some("DELL U2412"));
        assert_eq!(monitors[0].year, Some(2014));
        // No EDID line -> nothing.
        assert!(parse_ioreg_edid("nothing here").is_empty());
    }
}
