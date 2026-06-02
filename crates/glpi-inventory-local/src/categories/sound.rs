// SPDX-License-Identifier: GPL-2.0-only

//! Sound card inventory category (Linux `lspci`).
//!
//! Selects the audio-class devices from `lspci -mm` (Audio device / Multimedia)
//! as [`Sound`] cards for the GLPI `sounds` section. The selection parser is
//! pure and unit-tested; the live collector runs `lspci`.

use serde::Serialize;

use super::pci::{parse_lspci, Controller};

/// A sound card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Sound {
    /// Card name.
    pub name: String,
    /// Manufacturer / vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
}

/// `true` if a controller class denotes an audio device.
fn is_audio(controller: &Controller) -> bool {
    controller
        .controller_type
        .as_deref()
        .is_some_and(|t| t.contains("Audio") || t.contains("Multimedia"))
}

/// Selects the sound cards from `lspci -mm` output.
#[must_use]
pub fn parse_lspci_sound(text: &str) -> Vec<Sound> {
    parse_lspci(text)
        .into_iter()
        .filter(is_audio)
        .filter_map(|c| {
            c.name.map(|name| Sound {
                name,
                manufacturer: c.manufacturer,
            })
        })
        .collect()
}

/// Collects the live sound cards via `lspci -mm` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Sound> {
    match std::process::Command::new("lspci").arg("-mm").output() {
        Ok(output) if output.status.success() => {
            parse_lspci_sound(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live sound cards (macOS) from `SPAudioDataType`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Sound> {
    crate::sys::output("system_profiler", &["-json", "SPAudioDataType"])
        .map(|json| parse_macos_sound(&json))
        .unwrap_or_default()
}

/// Collects the live sound cards (Windows) from `Win32_SoundDevice`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Sound> {
    crate::sys::powershell(
        "Get-CimInstance Win32_SoundDevice | \
         Select-Object Name,Manufacturer | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_sound(&json))
    .unwrap_or_default()
}

/// Collects the live sound cards (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Sound> {
    Vec::new()
}

/// Parses a `Win32_SoundDevice` `ConvertTo-Json` result into the sound cards.
#[must_use]
pub fn parse_win_sound(json: &str) -> Vec<Sound> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            Some(Sound {
                name: crate::jsonutil::str_field(item, "Name")?,
                manufacturer: crate::jsonutil::str_field(item, "Manufacturer"),
            })
        })
        .collect()
}

/// Parses `system_profiler -json SPAudioDataType` (macOS) into the audio
/// devices, walking each provider's `_items`.
#[must_use]
pub fn parse_macos_sound(json: &str) -> Vec<Sound> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut sounds = Vec::new();
    let providers = value
        .get("SPAudioDataType")
        .and_then(serde_json::Value::as_array);
    for provider in providers.into_iter().flatten() {
        let items = provider
            .get("_items")
            .and_then(serde_json::Value::as_array)
            .map_or_else(|| std::slice::from_ref(provider), Vec::as_slice);
        for item in items {
            if let Some(name) = crate::jsonutil::str_field(item, "_name") {
                sounds.push(Sound {
                    name,
                    manufacturer: None,
                });
            }
        }
    }
    sounds
}

#[cfg(test)]
mod tests {
    use super::parse_lspci_sound;

    #[test]
    fn parses_windows_sound_json() {
        use super::parse_win_sound;
        let json = r#"[{"Name":"Realtek High Definition Audio","Manufacturer":"Realtek"}]"#;
        let sounds = parse_win_sound(json);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].name, "Realtek High Definition Audio");
        assert_eq!(sounds[0].manufacturer.as_deref(), Some("Realtek"));
    }

    #[test]
    fn parses_macos_sound_json() {
        use super::parse_macos_sound;
        let json = r#"{"SPAudioDataType":[{"_name":"Devices","_items":[
            {"_name":"MacBook Pro Speakers"},{"_name":"MacBook Pro Microphone"}]}]}"#;
        let sounds = parse_macos_sound(json);
        assert_eq!(sounds.len(), 2);
        assert_eq!(sounds[0].name, "MacBook Pro Speakers");
    }

    const LSPCI: &str = r#"00:02.0 "VGA compatible controller" "Intel Corporation" "UHD Graphics 630"
00:1f.3 "Audio device" "Intel Corporation" "Sunrise Point-LP HD Audio"
"#;

    #[test]
    fn selects_only_audio_devices() {
        let sounds = parse_lspci_sound(LSPCI);
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].name, "Sunrise Point-LP HD Audio");
        assert_eq!(sounds[0].manufacturer.as_deref(), Some("Intel Corporation"));
    }
}
