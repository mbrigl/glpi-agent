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

/// Collects the live sound cards (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Sound> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::parse_lspci_sound;

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
