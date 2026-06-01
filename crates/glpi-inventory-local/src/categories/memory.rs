// SPDX-License-Identifier: GPL-2.0-only

//! Memory inventory category (Linux `dmidecode -t 17`).
//!
//! Each populated `Memory Device` block from `dmidecode` becomes a
//! [`MemoryModule`]: capacity (normalized to MB), type, speed, slot locator,
//! manufacturer, serial and part number. Empty slots ("No Module Installed")
//! are skipped, and DMI placeholder values ("Unknown", "Not Specified", …) are
//! treated as absent. The parser is pure and unit-tested; the live collector
//! runs `dmidecode`.

use serde::Serialize;

/// A populated memory module (a DMI type 17 device).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MemoryModule {
    /// Capacity in megabytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u64>,
    /// Memory type (e.g. "DDR4").
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    /// Speed in MT/s (≈ MHz).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<u64>,
    /// Slot locator (e.g. "DIMM 0").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Module manufacturer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Serial number.
    #[serde(rename = "serialnumber", skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    /// Part number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Parses `dmidecode -t 17` output into the populated memory modules.
#[must_use]
pub fn parse_dmidecode_memory(text: &str) -> Vec<MemoryModule> {
    let mut modules = Vec::new();
    for block in text.split("\n\n") {
        if !block.lines().any(|l| l.trim() == "Memory Device") {
            continue;
        }
        let fields = block_fields(block);
        // Skip empty slots.
        match fields_get(&fields, "Size") {
            Some(size) if !size.contains("No Module") => {
                modules.push(MemoryModule {
                    capacity: parse_size_mb(size),
                    memory_type: clean(fields_get(&fields, "Type")),
                    speed: fields_get(&fields, "Speed").and_then(parse_speed),
                    caption: clean(fields_get(&fields, "Locator")),
                    manufacturer: clean(fields_get(&fields, "Manufacturer")),
                    serial_number: clean(fields_get(&fields, "Serial Number")),
                    model: clean(fields_get(&fields, "Part Number")),
                });
            }
            _ => {}
        }
    }
    modules
}

/// Collects the live memory modules via `dmidecode -t 17` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<MemoryModule> {
    match std::process::Command::new("dmidecode")
        .args(["-t", "17"])
        .output()
    {
        Ok(output) if output.status.success() => {
            parse_dmidecode_memory(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live memory modules (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<MemoryModule> {
    Vec::new()
}

/// Collects the indented `Key: Value` lines of a dmidecode block.
fn block_fields(block: &str) -> Vec<(String, String)> {
    block
        .lines()
        .filter(|l| l.starts_with('\t') || l.starts_with(' '))
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

fn fields_get<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Treats DMI placeholders as absent.
fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| {
            !v.is_empty()
                && !matches!(
                    *v,
                    "Unknown" | "Not Specified" | "<OUT OF SPEC>" | "None" | "Not Provided"
                )
        })
        .map(ToOwned::to_owned)
}

/// Parses a dmidecode size (`"16384 MB"`, `"16 GB"`) into megabytes.
fn parse_size_mb(size: &str) -> Option<u64> {
    let mut parts = size.split_whitespace();
    let value: u64 = parts.next()?.parse().ok()?;
    match parts.next()?.to_ascii_uppercase().as_str() {
        "MB" => Some(value),
        "GB" => Some(value * 1024),
        "TB" => Some(value * 1024 * 1024),
        "KB" => Some(value / 1024),
        _ => None,
    }
}

/// Parses a dmidecode speed (`"3200 MT/s"`, `"2666 MHz"`) into its number.
fn parse_speed(speed: &str) -> Option<u64> {
    speed.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_dmidecode_memory;

    const DMIDECODE: &str = "\
# dmidecode 3.4
Handle 0x0010, DMI type 17, 40 bytes
Memory Device
\tArray Handle: 0x000F
\tSize: 16384 MB
\tForm Factor: DIMM
\tLocator: DIMM 0
\tBank Locator: BANK 0
\tType: DDR4
\tSpeed: 3200 MT/s
\tManufacturer: Samsung
\tSerial Number: 12345678
\tPart Number: M378A2K43DB1

Handle 0x0011, DMI type 17, 40 bytes
Memory Device
\tArray Handle: 0x000F
\tSize: No Module Installed
\tLocator: DIMM 1
\tType: Unknown
\tSpeed: Unknown
\tManufacturer: Not Specified
";

    #[test]
    fn parses_populated_module_and_skips_empty_slot() {
        let modules = parse_dmidecode_memory(DMIDECODE);
        assert_eq!(modules.len(), 1);
        let m = &modules[0];
        assert_eq!(m.capacity, Some(16384));
        assert_eq!(m.memory_type.as_deref(), Some("DDR4"));
        assert_eq!(m.speed, Some(3200));
        assert_eq!(m.caption.as_deref(), Some("DIMM 0"));
        assert_eq!(m.manufacturer.as_deref(), Some("Samsung"));
        assert_eq!(m.serial_number.as_deref(), Some("12345678"));
        assert_eq!(m.model.as_deref(), Some("M378A2K43DB1"));
    }

    #[test]
    fn normalizes_gb_sizes() {
        let block = "Handle, DMI type 17\nMemory Device\n\tSize: 8 GB\n\tLocator: DIMM A\n";
        let modules = parse_dmidecode_memory(block);
        assert_eq!(modules[0].capacity, Some(8192));
    }

    #[test]
    fn placeholder_values_become_none() {
        let block =
            "x\nMemory Device\n\tSize: 4096 MB\n\tType: Unknown\n\tManufacturer: Not Specified\n";
        let modules = parse_dmidecode_memory(block);
        assert_eq!(modules[0].memory_type, None);
        assert_eq!(modules[0].manufacturer, None);
    }

    #[test]
    fn no_memory_devices_yields_empty() {
        assert!(parse_dmidecode_memory("Handle\nBIOS Information\n\tVendor: x\n").is_empty());
    }
}
