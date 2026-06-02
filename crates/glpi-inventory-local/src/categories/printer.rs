// SPDX-License-Identifier: GPL-2.0-only

//! Printer inventory category (Linux CUPS via `lpstat -p`).
//!
//! Parses `lpstat -p` lines (`printer NAME is idle. …` / `printer NAME disabled
//! …`) into [`Printer`] records (name + coarse status) for the GLPI `printers`
//! section. The parser is pure and unit-tested; the live collector runs
//! `lpstat`.

use serde::Serialize;

/// A locally configured printer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Printer {
    /// Queue name.
    pub name: String,
    /// Coarse status ("Idle", "Printing", "Disabled").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Driver / make-and-model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// Device URI / port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    /// Serial number, when the device URI carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
}

/// Parses `lpstat -p` output into printers.
#[must_use]
pub fn parse_lpstat(text: &str) -> Vec<Printer> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("printer ")?;
            let name = rest.split_whitespace().next()?;
            if name.is_empty() {
                return None;
            }
            Some(Printer {
                name: name.to_owned(),
                status: status_of(line),
                ..Printer::default()
            })
        })
        .collect()
}

/// Collects the live printers via `lpstat -p` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Printer> {
    match std::process::Command::new("lpstat").arg("-p").output() {
        Ok(output) if output.status.success() => {
            parse_lpstat(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live printers (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Printer> {
    Vec::new()
}

/// Derives a coarse status from an `lpstat -p` line.
fn status_of(line: &str) -> Option<String> {
    if line.contains("disabled") {
        Some("Disabled".to_owned())
    } else if line.contains("printing") {
        Some("Printing".to_owned())
    } else if line.contains("idle") {
        Some("Idle".to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_lpstat;

    const LPSTAT: &str = "\
printer HP_LaserJet is idle.  enabled since Mon 01 Jan 2024 10:00:00 AM UTC
printer Office_MFP now printing Office_MFP-42.  enabled since ...
printer Old_Printer disabled since Sun 31 Dec 2023 ...
";

    #[test]
    fn parses_printers_and_status() {
        let printers = parse_lpstat(LPSTAT);
        assert_eq!(printers.len(), 3);
        assert_eq!(printers[0].name, "HP_LaserJet");
        assert_eq!(printers[0].status.as_deref(), Some("Idle"));
        assert_eq!(printers[1].name, "Office_MFP");
        assert_eq!(printers[1].status.as_deref(), Some("Printing"));
        assert_eq!(printers[2].name, "Old_Printer");
        assert_eq!(printers[2].status.as_deref(), Some("Disabled"));
    }

    #[test]
    fn no_printers_yields_empty() {
        assert!(parse_lpstat("no destinations added.\n").is_empty());
    }
}
