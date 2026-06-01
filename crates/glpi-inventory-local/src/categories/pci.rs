// SPDX-License-Identifier: GPL-2.0-only

//! PCI controller inventory category (Linux `lspci -mm`).
//!
//! `lspci -mm` prints one device per line with the slot followed by
//! double-quoted class, vendor and device names, which `lspci` already resolves
//! from the PCI id database. Each device becomes a [`Controller`] in the GLPI
//! `controllers` section. The quoted-field parser is pure and unit-tested; the
//! live collector runs `lspci`.

use serde::Serialize;

/// A PCI controller / device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Controller {
    /// Device name (the resolved device description).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Vendor / manufacturer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Device class (e.g. "VGA compatible controller").
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub controller_type: Option<String>,
    /// PCI slot (e.g. "00:02.0").
    #[serde(rename = "pcislot", skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}

/// Parses `lspci -mm` output into controllers.
///
/// Each line is `SLOT "CLASS" "VENDOR" "DEVICE" …`; the first three quoted
/// fields are taken.
#[must_use]
pub fn parse_lspci(text: &str) -> Vec<Controller> {
    text.lines()
        .filter_map(|line| {
            let slot = line.split('"').next()?.trim();
            if slot.is_empty() {
                return None;
            }
            // Quoted contents are the odd-indexed pieces of a split on '"'.
            let quoted: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
            Some(Controller {
                controller_type: non_empty(quoted.first().copied()),
                manufacturer: non_empty(quoted.get(1).copied()),
                name: non_empty(quoted.get(2).copied()),
                slot: Some(slot.to_owned()),
            })
        })
        .collect()
}

/// Collects the live PCI controllers via `lspci -mm` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Controller> {
    match std::process::Command::new("lspci").arg("-mm").output() {
        Ok(output) if output.status.success() => {
            parse_lspci(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live PCI controllers (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Controller> {
    Vec::new()
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::parse_lspci;

    const LSPCI: &str = r#"00:00.0 "Host bridge" "Intel Corporation" "Device 3e34" -r0c "Dell" "Device 087c"
00:02.0 "VGA compatible controller" "Intel Corporation" "UHD Graphics 630 (Mobile)"
00:1f.3 "Audio device" "Intel Corporation" "Sunrise Point-LP HD Audio"
"#;

    #[test]
    fn parses_controllers_with_quoted_fields() {
        let controllers = parse_lspci(LSPCI);
        assert_eq!(controllers.len(), 3);

        let host = &controllers[0];
        assert_eq!(host.slot.as_deref(), Some("00:00.0"));
        assert_eq!(host.controller_type.as_deref(), Some("Host bridge"));
        assert_eq!(host.manufacturer.as_deref(), Some("Intel Corporation"));
        assert_eq!(host.name.as_deref(), Some("Device 3e34"));

        let vga = &controllers[1];
        assert_eq!(
            vga.controller_type.as_deref(),
            Some("VGA compatible controller")
        );
        assert_eq!(vga.name.as_deref(), Some("UHD Graphics 630 (Mobile)"));
    }

    #[test]
    fn empty_input_yields_no_controllers() {
        assert!(parse_lspci("").is_empty());
    }
}
