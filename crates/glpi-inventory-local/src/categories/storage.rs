// SPDX-License-Identifier: GPL-2.0-only

//! Storage inventory category (Linux `lsblk`).
//!
//! Uses `lsblk`'s key/value pair output (`-P`), which quotes each value so
//! model strings with spaces parse unambiguously. Only whole devices (`disk`,
//! `rom`) are reported, not partitions or loop devices. Sizes are converted
//! from bytes to MB. The pair parser is pure and unit-tested; the live
//! collector runs `lsblk`.

use serde::Serialize;

/// A storage device (a whole disk or optical drive).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Storage {
    /// Device name (e.g. "sda").
    pub name: String,
    /// Device type ("disk", "rom").
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub disk_type: Option<String>,
    /// Capacity in megabytes.
    #[serde(rename = "disksize", skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Model name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Serial number.
    #[serde(rename = "serialnumber", skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// Manufacturer / vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Firmware revision (from `smartctl`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
}

/// Identity fields read from `smartctl -i`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartInfo {
    /// Model name (`Device Model` / `Model Number`).
    pub model: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// Firmware revision.
    pub firmware: Option<String>,
}

/// Parses `smartctl -i` output for the drive's model / serial / firmware.
#[must_use]
pub fn parse_smartctl_info(text: &str) -> SmartInfo {
    let get = |key: &str| {
        text.lines()
            .find_map(|line| line.split_once(':').filter(|(k, _)| k.trim() == key))
            .map(|(_, v)| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    SmartInfo {
        // ATA drives report "Device Model"; NVMe reports "Model Number".
        model: get("Device Model").or_else(|| get("Model Number")),
        serial: get("Serial Number"),
        firmware: get("Firmware Version"),
    }
}

/// Parses `lsblk -P` (key="value" pairs) output into whole storage devices.
///
/// Expects fields `NAME`, `TYPE`, `SIZE` (bytes), `MODEL`, `SERIAL`, `VENDOR`.
/// Partitions and loop devices are skipped.
#[must_use]
pub fn parse_lsblk(text: &str) -> Vec<Storage> {
    text.lines()
        .map(parse_pairs)
        .filter(|pairs| matches!(pairs_get(pairs, "TYPE"), Some("disk" | "rom")))
        .filter_map(|pairs| {
            let name = pairs_get(&pairs, "NAME")?.to_owned();
            if name.is_empty() {
                return None;
            }
            Some(Storage {
                name,
                disk_type: non_empty(pairs_get(&pairs, "TYPE")),
                size: pairs_get(&pairs, "SIZE")
                    .and_then(|s| s.parse::<u64>().ok())
                    .filter(|b| *b > 0)
                    .map(|bytes| bytes / 1_048_576),
                model: non_empty(pairs_get(&pairs, "MODEL")),
                serial: non_empty(pairs_get(&pairs, "SERIAL")),
                manufacturer: non_empty(pairs_get(&pairs, "VENDOR")),
                firmware: None,
            })
        })
        .collect()
}

/// Collects the live storage devices via `lsblk` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Storage> {
    let mut storages = match std::process::Command::new("lsblk")
        .args(["-dnPb", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,VENDOR"])
        .output()
    {
        Ok(output) if output.status.success() => {
            parse_lsblk(&String::from_utf8_lossy(&output.stdout))
        }
        _ => return Vec::new(),
    };
    // Enrich with smartctl (firmware, and serial/model when lsblk lacked them).
    for storage in &mut storages {
        if let Some(text) = run_smartctl(&storage.name) {
            let info = parse_smartctl_info(&text);
            storage.firmware = storage.firmware.take().or(info.firmware);
            storage.serial = storage.serial.take().or(info.serial);
            storage.model = storage.model.take().or(info.model);
        }
    }
    storages
}

/// Runs `smartctl -i /dev/<name>`, returning stdout on success.
#[cfg(target_os = "linux")]
fn run_smartctl(name: &str) -> Option<String> {
    let output = std::process::Command::new("smartctl")
        .arg("-i")
        .arg(format!("/dev/{name}"))
        .output()
        .ok()?;
    // smartctl uses bit-coded exit statuses; stdout is still valid when the
    // low bits (command/usage errors) are clear, so just require some output.
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    (!text.trim().is_empty()).then_some(text)
}

/// Collects the live storage devices (macOS) from `system_profiler`'s NVMe and
/// SATA data types.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Storage> {
    let mut disks = Vec::new();
    for data_type in ["SPNVMeDataType", "SPSerialATADataType"] {
        if let Some(json) = crate::sys::output("system_profiler", &["-json", data_type]) {
            disks.extend(parse_macos_storage(&json));
        }
    }
    disks
}

/// Collects the live storage devices (Windows) from `Win32_DiskDrive`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Storage> {
    crate::sys::powershell(
        "Get-CimInstance Win32_DiskDrive | Select-Object \
         Model,Size,SerialNumber,MediaType,FirmwareRevision,Manufacturer | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_storage(&json))
    .unwrap_or_default()
}

/// Collects the live storage devices (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Storage> {
    Vec::new()
}

/// Parses a `Win32_DiskDrive` `ConvertTo-Json` result into the storage devices.
#[must_use]
pub fn parse_win_storage(json: &str) -> Vec<Storage> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .map(|item| {
            let model = crate::jsonutil::str_field(item, "Model");
            let media = crate::jsonutil::str_field(item, "MediaType").unwrap_or_default();
            let disk_type = if media.to_ascii_lowercase().contains("removable") {
                "removable"
            } else {
                "disk"
            };
            Storage {
                name: model.clone().unwrap_or_else(|| "disk".to_owned()),
                disk_type: Some(disk_type.to_owned()),
                size: crate::jsonutil::u64_field(item, "Size").map(|b| b / (1024 * 1024)),
                model,
                serial: crate::jsonutil::str_field(item, "SerialNumber"),
                // CIM reports generic disks as "(Standard disk drives)"; drop it.
                manufacturer: crate::jsonutil::str_field(item, "Manufacturer")
                    .filter(|m| !m.starts_with('(')),
                firmware: crate::jsonutil::str_field(item, "FirmwareRevision"),
            }
        })
        .collect()
}

/// Parses `system_profiler -json SPNVMeDataType`/`SPSerialATADataType` (macOS)
/// into storage devices. Best-effort: walks each controller's `_items` for the
/// actual drives and tolerates the schema differences between the data types.
#[must_use]
pub fn parse_macos_storage(json: &str) -> Vec<Storage> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut disks = Vec::new();
    let Some(object) = value.as_object() else {
        return disks;
    };
    for entries in object.values() {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries {
            // A controller groups its drives under `_items`; otherwise the entry
            // is itself a drive.
            match entry.get("_items").and_then(serde_json::Value::as_array) {
                Some(items) => disks.extend(items.iter().filter_map(macos_disk)),
                None => disks.extend(macos_disk(entry)),
            }
        }
    }
    disks
}

/// Builds a [`Storage`] from a macOS drive object, or `None` if the object is a
/// controller (no `bsd_name`/`device_model`).
fn macos_disk(item: &serde_json::Value) -> Option<Storage> {
    if item.get("bsd_name").is_none() && item.get("device_model").is_none() {
        return None;
    }
    let model = crate::jsonutil::str_field(item, "device_model")
        .or_else(|| crate::jsonutil::str_field(item, "_name"));
    let name = crate::jsonutil::str_field(item, "bsd_name")
        .or_else(|| model.clone())
        .unwrap_or_else(|| "disk".to_owned());
    Some(Storage {
        name,
        disk_type: Some("disk".to_owned()),
        size: crate::jsonutil::str_field(item, "size").and_then(|s| parse_macos_size(&s)),
        model,
        serial: crate::jsonutil::str_field(item, "device_serial"),
        manufacturer: None,
        firmware: crate::jsonutil::str_field(item, "device_revision"),
    })
}

/// Parses a `system_profiler` size string (`"500.28 GB"`, `"1 TB"`) into MB
/// (decimal units, as macOS reports drive capacities).
fn parse_macos_size(size: &str) -> Option<u64> {
    let mut parts = size.split_whitespace();
    let value: f64 = parts.next()?.replace(',', ".").parse().ok()?;
    let mb = match parts.next().unwrap_or("").to_ascii_uppercase().as_str() {
        "TB" => value * 1_000_000.0,
        "GB" => value * 1000.0,
        "MB" => value,
        _ => return None,
    };
    Some(mb as u64)
}

/// Parses one `KEY="value" KEY2="value"` line into pairs.
fn parse_pairs(line: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut rest = line.trim();
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim().to_owned();
        let after = rest[eq + 1..].trim_start();
        let Some(body) = after.strip_prefix('"') else {
            break;
        };
        let Some(end) = body.find('"') else {
            break;
        };
        pairs.push((key, body[..end].to_owned()));
        rest = body[end + 1..].trim_start();
    }
    pairs
}

fn pairs_get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::parse_lsblk;

    const LSBLK: &str = r#"NAME="sda" TYPE="disk" SIZE="256060514304" MODEL="Samsung SSD 860 EVO" SERIAL="S3Z1NB0K12" VENDOR="ATA"
NAME="sda1" TYPE="part" SIZE="255900000000" MODEL="" SERIAL="" VENDOR=""
NAME="sr0" TYPE="rom" SIZE="1073741824" MODEL="DVD-RW" SERIAL="" VENDOR="HL-DT-ST"
NAME="loop0" TYPE="loop" SIZE="65536" MODEL="" SERIAL="" VENDOR=""
"#;

    #[test]
    fn parses_disks_and_roms_skipping_partitions_and_loops() {
        let storages = parse_lsblk(LSBLK);
        assert_eq!(storages.len(), 2);

        let sda = &storages[0];
        assert_eq!(sda.name, "sda");
        assert_eq!(sda.disk_type.as_deref(), Some("disk"));
        assert_eq!(sda.model.as_deref(), Some("Samsung SSD 860 EVO")); // spaces preserved
        assert_eq!(sda.serial.as_deref(), Some("S3Z1NB0K12"));
        assert_eq!(sda.manufacturer.as_deref(), Some("ATA"));
        assert_eq!(sda.size, Some(256_060_514_304 / 1_048_576));

        let sr0 = &storages[1];
        assert_eq!(sr0.name, "sr0");
        assert_eq!(sr0.disk_type.as_deref(), Some("rom"));
        // Empty serial/vendor become None.
        assert_eq!(sr0.serial, None);
    }

    #[test]
    fn empty_input_yields_no_storage() {
        assert!(parse_lsblk("").is_empty());
    }

    #[test]
    fn parses_smartctl_info_ata_and_nvme() {
        use super::parse_smartctl_info;

        let ata = "\
smartctl 7.3 2022-02-28 r5338
Device Model:     Samsung SSD 860 EVO 250GB
Serial Number:    S3Z1NB0K123456
Firmware Version: RVT01B6Q
User Capacity:    250,059,350,016 bytes
";
        let info = parse_smartctl_info(ata);
        assert_eq!(info.model.as_deref(), Some("Samsung SSD 860 EVO 250GB"));
        assert_eq!(info.serial.as_deref(), Some("S3Z1NB0K123456"));
        assert_eq!(info.firmware.as_deref(), Some("RVT01B6Q"));

        // NVMe uses "Model Number" instead of "Device Model".
        let nvme =
            "Model Number:    WD Blue SN570\nSerial Number:    24xx\nFirmware Version: 1.0\n";
        let info = parse_smartctl_info(nvme);
        assert_eq!(info.model.as_deref(), Some("WD Blue SN570"));
        assert_eq!(info.firmware.as_deref(), Some("1.0"));
    }

    #[test]
    fn parses_windows_disk_drive_json() {
        use super::parse_win_storage;
        let json = r#"[{"Model":"Samsung SSD 970 EVO","Size":"500107862016","SerialNumber":"S123 ",
            "MediaType":"Fixed hard disk media","FirmwareRevision":"1B2QEXM7",
            "Manufacturer":"(Standard disk drives)"}]"#;
        let disks = parse_win_storage(json);
        assert_eq!(disks.len(), 1);
        let d = &disks[0];
        assert_eq!(d.name, "Samsung SSD 970 EVO");
        assert_eq!(d.disk_type.as_deref(), Some("disk"));
        assert_eq!(d.size, Some(500_107_862_016 / 1_048_576));
        assert_eq!(d.serial.as_deref(), Some("S123")); // trimmed
        assert_eq!(d.firmware.as_deref(), Some("1B2QEXM7"));
        // Generic CIM manufacturer dropped.
        assert_eq!(d.manufacturer, None);
        assert!(parse_win_storage("bad").is_empty());
    }

    #[test]
    fn parses_macos_storage_json() {
        use super::parse_macos_storage;
        // A controller groups the drive under `_items`.
        let json = r#"{"SPNVMeDataType":[{"_name":"APPLE SSD Controller",
            "_items":[{"_name":"APPLE SSD AP0512","bsd_name":"disk0","device_model":"APPLE SSD AP0512",
            "size":"500.28 GB","device_serial":"S0001","device_revision":"1.0"}]}]}"#;
        let disks = parse_macos_storage(json);
        assert_eq!(disks.len(), 1);
        let d = &disks[0];
        assert_eq!(d.name, "disk0");
        assert_eq!(d.model.as_deref(), Some("APPLE SSD AP0512"));
        assert_eq!(d.size, Some(500_280)); // 500.28 GB (decimal) -> MB
        assert_eq!(d.serial.as_deref(), Some("S0001"));
        assert_eq!(d.firmware.as_deref(), Some("1.0"));
        // The bare controller (no bsd_name/device_model) is not a disk.
        let only_controller = r#"{"SPNVMeDataType":[{"_name":"Controller"}]}"#;
        assert!(parse_macos_storage(only_controller).is_empty());
    }
}
