// SPDX-License-Identifier: GPL-2.0-only

//! Video controller (GPU) inventory category (Linux `lspci`).
//!
//! Selects the display-class devices from `lspci -mm` (VGA / 3D / Display
//! controllers) and reports them as [`Video`] cards for the GLPI `videos`
//! section. The selection parser is pure and unit-tested; the live collector
//! runs `lspci`.

use serde::Serialize;

use super::pci::{parse_lspci, Controller};

/// A video / graphics controller.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Video {
    /// Card name.
    pub name: String,
    /// Manufacturer / vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
}

/// `true` if a controller class denotes a display adapter.
fn is_display(controller: &Controller) -> bool {
    controller
        .controller_type
        .as_deref()
        .is_some_and(|t| t.contains("VGA") || t.contains("3D") || t.contains("Display"))
}

/// Selects the video controllers from `lspci -mm` output.
#[must_use]
pub fn parse_lspci_video(text: &str) -> Vec<Video> {
    parse_lspci(text)
        .into_iter()
        .filter(is_display)
        .filter_map(|c| {
            c.name.map(|name| Video {
                name,
                manufacturer: c.manufacturer,
            })
        })
        .collect()
}

/// Collects the live video controllers via `lspci -mm` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Video> {
    match std::process::Command::new("lspci").arg("-mm").output() {
        Ok(output) if output.status.success() => {
            parse_lspci_video(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live video controllers (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Video> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::parse_lspci_video;

    const LSPCI: &str = r#"00:00.0 "Host bridge" "Intel Corporation" "Device 3e34"
00:02.0 "VGA compatible controller" "Intel Corporation" "UHD Graphics 630"
01:00.0 "3D controller" "NVIDIA Corporation" "GP107M"
00:1f.3 "Audio device" "Intel Corporation" "HD Audio"
"#;

    #[test]
    fn selects_only_display_controllers() {
        let videos = parse_lspci_video(LSPCI);
        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].name, "UHD Graphics 630");
        assert_eq!(videos[0].manufacturer.as_deref(), Some("Intel Corporation"));
        assert_eq!(videos[1].name, "GP107M");
        assert_eq!(
            videos[1].manufacturer.as_deref(),
            Some("NVIDIA Corporation")
        );
    }

    #[test]
    fn no_display_controllers_yields_empty() {
        let videos =
            parse_lspci_video("00:1f.3 \"Audio device\" \"Intel Corporation\" \"HD Audio\"\n");
        assert!(videos.is_empty());
    }
}
