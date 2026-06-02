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

/// Collects the live video controllers (macOS) from `SPDisplaysDataType`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Video> {
    crate::sys::output("system_profiler", &["-json", "SPDisplaysDataType"])
        .map(|json| parse_macos_video(&json))
        .unwrap_or_default()
}

/// Collects the live video controllers (Windows) from `Win32_VideoController`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Video> {
    crate::sys::powershell(
        "Get-CimInstance Win32_VideoController | \
         Select-Object Name,AdapterCompatibility | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_video(&json))
    .unwrap_or_default()
}

/// Collects the live video controllers (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Video> {
    Vec::new()
}

/// Parses a `Win32_VideoController` `ConvertTo-Json` result into the GPUs.
#[must_use]
pub fn parse_win_video(json: &str) -> Vec<Video> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            Some(Video {
                name: crate::jsonutil::str_field(item, "Name")?,
                manufacturer: crate::jsonutil::str_field(item, "AdapterCompatibility"),
            })
        })
        .collect()
}

/// Parses `system_profiler -json SPDisplaysDataType` (macOS) into the GPUs.
#[must_use]
pub fn parse_macos_video(json: &str) -> Vec<Video> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    value
        .get("SPDisplaysDataType")
        .and_then(serde_json::Value::as_array)
        .map(|gpus| {
            gpus.iter()
                .filter_map(|item| {
                    Some(Video {
                        name: crate::jsonutil::str_field(item, "_name")?,
                        manufacturer: crate::jsonutil::str_field(item, "spdisplays_vendor"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::parse_lspci_video;

    #[test]
    fn parses_windows_video_json() {
        use super::parse_win_video;
        let json =
            r#"[{"Name":"Intel UHD Graphics 630","AdapterCompatibility":"Intel Corporation"}]"#;
        let videos = parse_win_video(json);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].name, "Intel UHD Graphics 630");
        assert_eq!(videos[0].manufacturer.as_deref(), Some("Intel Corporation"));
    }

    #[test]
    fn parses_macos_video_json() {
        use super::parse_macos_video;
        let json =
            r#"{"SPDisplaysDataType":[{"_name":"Apple M1 Pro","spdisplays_vendor":"Apple"}]}"#;
        let videos = parse_macos_video(json);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].name, "Apple M1 Pro");
    }

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
