// SPDX-License-Identifier: GPL-2.0-only

//! Printer inventory category (Linux CUPS via `lpstat`).
//!
//! [`parse_printers`] merges `lpstat -l -p` (queue, status, description and —
//! when present — make-and-model) with `lpstat -v` (the device URI, from which
//! a `serial=`/`uuid=` is lifted) into rich [`Printer`] records for the GLPI
//! `printers` section, with no dependency on a CUPS library. [`parse_lpstat`]
//! remains for the plain name+status case. The parsers are pure and
//! unit-tested; the live collector runs `lpstat`.

use std::collections::BTreeMap;

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

/// Parses `lpstat -l -p` (status, description, make-and-model) and `lpstat -v`
/// (device URIs) into rich printers. The URI becomes the `port`, and a
/// `serial=`/`uuid=` parameter in it the `serial`.
#[must_use]
pub fn parse_printers(status_output: &str, devices_output: &str) -> Vec<Printer> {
    let devices = parse_lpstat_devices(devices_output);
    let mut printers: Vec<Printer> = Vec::new();
    for line in status_output.lines() {
        if let Some(rest) = line.strip_prefix("printer ") {
            let Some(name) = rest.split_whitespace().next() else {
                continue;
            };
            if !name.is_empty() {
                printers.push(Printer {
                    name: name.to_owned(),
                    status: status_of(line),
                    ..Printer::default()
                });
            }
        } else if let Some(printer) = printers.last_mut() {
            // Indented detail lines belong to the most recent printer.
            let detail = line.trim();
            if let Some(value) = detail.strip_prefix("Description:") {
                printer.description = non_empty(value);
            } else if let Some(value) = detail.strip_prefix("Make and Model:") {
                printer.driver = non_empty(value);
            }
        }
    }
    for printer in &mut printers {
        if let Some(uri) = devices.get(&printer.name) {
            printer.serial = serial_from_device_uri(uri);
            printer.port = Some(uri.clone());
        }
    }
    printers
}

/// Parses `lpstat -v` lines (`device for NAME: URI`) into a name→URI map.
#[must_use]
pub fn parse_lpstat_devices(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("device for ")?;
            let (name, uri) = rest.split_once(':')?;
            let uri = uri.trim();
            (!uri.is_empty()).then(|| (name.trim().to_owned(), uri.to_owned()))
        })
        .collect()
}

/// Extracts a `serial=`/`uuid=` value from a CUPS device URI's query string.
#[must_use]
pub fn serial_from_device_uri(uri: &str) -> Option<String> {
    let query = uri.split_once('?')?.1;
    query
        .split('&')
        .find_map(|param| {
            param
                .strip_prefix("serial=")
                .or_else(|| param.strip_prefix("uuid="))
        })
        .map(str::to_owned)
}

/// Trims `value` and maps an empty result to `None`.
fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Collects the live printers via `lpstat -l -p` + `lpstat -v` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Printer> {
    let status = run_lpstat(&["-l", "-p"]);
    let devices = run_lpstat(&["-v"]);
    parse_printers(&status, &devices)
}

/// Runs `lpstat <args>`, returning stdout on success (empty otherwise).
#[cfg(target_os = "linux")]
fn run_lpstat(args: &[&str]) -> String {
    match std::process::Command::new("lpstat").args(args).output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        _ => String::new(),
    }
}

/// Collects the live printers via CUPS `lpstat` (macOS; same CUPS as Linux).
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Printer> {
    let status = crate::sys::output("lpstat", &["-l", "-p"]).unwrap_or_default();
    let devices = crate::sys::output("lpstat", &["-v"]).unwrap_or_default();
    parse_printers(&status, &devices)
}

/// Collects the live printers (Windows) from `Win32_Printer`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Printer> {
    crate::sys::powershell(
        "Get-CimInstance Win32_Printer | \
         Select-Object Name,DriverName,PortName,Comment | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_printers(&json))
    .unwrap_or_default()
}

/// Collects the live printers (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Printer> {
    Vec::new()
}

/// Parses a `Win32_Printer` `ConvertTo-Json` result into the printers.
#[must_use]
pub fn parse_win_printers(json: &str) -> Vec<Printer> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            Some(Printer {
                name: crate::jsonutil::str_field(item, "Name")?,
                status: None,
                description: crate::jsonutil::str_field(item, "Comment"),
                driver: crate::jsonutil::str_field(item, "DriverName"),
                port: crate::jsonutil::str_field(item, "PortName"),
                serial: None,
            })
        })
        .collect()
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

    #[test]
    fn parse_printers_merges_status_description_and_device_uri() {
        let status = "\
printer Reception is idle.  enabled since Mon 01 Jan 2024 09:00:00 AM CET
\tDescription: Reception desk
\tMake and Model: HP LaserJet Pro M404
\tLocation: Lobby
printer Warehouse disabled since ...
\tDescription: Loading bay
";
        let devices = "\
device for Reception: ipp://printer.local/ipp/print?serial=ABC123
device for Warehouse: socket://10.0.0.7
";
        let printers = super::parse_printers(status, devices);
        assert_eq!(printers.len(), 2);

        let reception = &printers[0];
        assert_eq!(reception.name, "Reception");
        assert_eq!(reception.status.as_deref(), Some("Idle"));
        assert_eq!(reception.description.as_deref(), Some("Reception desk"));
        assert_eq!(reception.driver.as_deref(), Some("HP LaserJet Pro M404"));
        assert_eq!(
            reception.port.as_deref(),
            Some("ipp://printer.local/ipp/print?serial=ABC123")
        );
        assert_eq!(reception.serial.as_deref(), Some("ABC123"));

        let warehouse = &printers[1];
        assert_eq!(warehouse.status.as_deref(), Some("Disabled"));
        assert_eq!(warehouse.description.as_deref(), Some("Loading bay"));
        assert_eq!(warehouse.port.as_deref(), Some("socket://10.0.0.7"));
        assert_eq!(warehouse.serial, None);
    }

    #[test]
    fn parses_devices_and_serial_from_uri() {
        let devices = super::parse_lpstat_devices("device for X: usb://HP/LJ?serial=SN9&foo=1\n");
        assert_eq!(
            devices.get("X").map(String::as_str),
            Some("usb://HP/LJ?serial=SN9&foo=1")
        );
        assert_eq!(
            super::serial_from_device_uri("usb://HP/LJ?serial=SN9&foo=1").as_deref(),
            Some("SN9")
        );
        assert_eq!(super::serial_from_device_uri("socket://10.0.0.7"), None);
    }

    #[test]
    fn parses_windows_printer_json() {
        use super::parse_win_printers;
        let json = r#"[{"Name":"HP LaserJet","DriverName":"HP Universal","PortName":"USB001",
            "Comment":"Front desk"},{"DriverName":"orphan"}]"#;
        let printers = parse_win_printers(json);
        // The entry without a Name is skipped.
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0].name, "HP LaserJet");
        assert_eq!(printers[0].driver.as_deref(), Some("HP Universal"));
        assert_eq!(printers[0].port.as_deref(), Some("USB001"));
        assert_eq!(printers[0].description.as_deref(), Some("Front desk"));
    }
}
