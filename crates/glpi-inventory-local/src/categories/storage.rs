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
            })
        })
        .collect()
}

/// Collects the live storage devices via `lsblk` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Storage> {
    match std::process::Command::new("lsblk")
        .args(["-dnPb", "-o", "NAME,TYPE,SIZE,MODEL,SERIAL,VENDOR"])
        .output()
    {
        Ok(output) if output.status.success() => {
            parse_lsblk(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live storage devices (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Storage> {
    Vec::new()
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
}
