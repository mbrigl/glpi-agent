// SPDX-License-Identifier: GPL-2.0-only

//! CPU inventory category (Linux `/proc/cpuinfo`).
//!
//! `/proc/cpuinfo` lists one block per *logical* processor. This groups them
//! into *physical* CPUs by `physical id`, reporting per socket: model name,
//! manufacturer (from `vendor_id`), nominal speed (from the model string's
//! `@ x.xxGHz`, falling back to `cpu MHz`), core count (`cpu cores`) and thread
//! count (the number of logical processors on that socket).

use std::collections::BTreeMap;

use serde::Serialize;

/// A physical CPU.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Cpu {
    /// Marketing model name (`model name`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Manufacturer, normalized from `vendor_id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Nominal speed in MHz.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<u64>,
    /// Number of physical cores (`cpu cores`).
    #[serde(rename = "core", skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    /// Number of logical processors (threads) on this socket.
    #[serde(rename = "thread", skip_serializing_if = "Option::is_none")]
    pub threads: Option<u32>,
}

/// Collects the live physical CPUs from `/proc/cpuinfo` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Cpu> {
    std::fs::read_to_string("/proc/cpuinfo")
        .map(|text| parse_cpuinfo(&text))
        .unwrap_or_default()
}

/// Collects the live physical CPUs (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Cpu> {
    Vec::new()
}

/// Parses `/proc/cpuinfo` into the physical CPUs, ordered by `physical id`.
#[must_use]
pub fn parse_cpuinfo(text: &str) -> Vec<Cpu> {
    // socket id -> (representative fields, logical-processor count)
    let mut sockets: BTreeMap<u64, (Block, u32)> = BTreeMap::new();
    for block in blocks(text) {
        if block.is_empty() {
            continue;
        }
        let socket_id = block
            .get("physical id")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let entry = sockets
            .entry(socket_id)
            .or_insert_with(|| (block.clone(), 0));
        entry.1 += 1;
    }

    sockets
        .into_values()
        .map(|(fields, threads)| Cpu {
            name: fields.get("model name").cloned(),
            manufacturer: fields.get("vendor_id").map(|v| normalize_vendor(v)),
            speed: speed_mhz(&fields),
            cores: fields.get("cpu cores").and_then(|v| v.parse().ok()),
            threads: Some(threads),
        })
        .collect()
}

type Block = std::collections::HashMap<String, String>;

/// Splits `/proc/cpuinfo` into per-processor blocks of `key: value` fields.
fn blocks(text: &str) -> Vec<Block> {
    let mut result = Vec::new();
    let mut current = Block::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                result.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            current.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

/// Normalizes a `vendor_id` to a friendly manufacturer name.
fn normalize_vendor(vendor: &str) -> String {
    match vendor {
        "GenuineIntel" => "Intel".to_owned(),
        "AuthenticAMD" => "AMD".to_owned(),
        other => other.to_owned(),
    }
}

/// Determines the nominal speed: from the model string's `@ x.xxGHz` if present,
/// otherwise the rounded `cpu MHz`.
fn speed_mhz(fields: &Block) -> Option<u64> {
    if let Some(speed) = fields.get("model name").and_then(|m| speed_from_model(m)) {
        return Some(speed);
    }
    fields
        .get("cpu MHz")
        .and_then(|v| v.parse::<f64>().ok())
        .map(|mhz| mhz.round() as u64)
}

/// Parses a clock speed from a model string such as `... @ 2.50GHz`.
fn speed_from_model(model: &str) -> Option<u64> {
    let at = model.rfind('@')?;
    let rest = model[at + 1..].trim();
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let value: f64 = number.parse().ok()?;
    let unit = rest[number.len()..].trim().to_ascii_uppercase();
    if unit.starts_with("GHZ") {
        Some((value * 1000.0).round() as u64)
    } else if unit.starts_with("MHZ") {
        Some(value.round() as u64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cpuinfo;

    // Two sockets (physical id 0 and 1), two logical processors each, 2 cores
    // each; current MHz is scaled but the model names the nominal 2.50GHz.
    const CPUINFO: &str = "\
processor\t: 0
vendor_id\t: GenuineIntel
model name\t: Intel(R) Xeon(R) CPU E5-2640 0 @ 2.50GHz
cpu MHz\t\t: 1200.000
physical id\t: 0
cpu cores\t: 2

processor\t: 1
vendor_id\t: GenuineIntel
model name\t: Intel(R) Xeon(R) CPU E5-2640 0 @ 2.50GHz
physical id\t: 0
cpu cores\t: 2

processor\t: 2
vendor_id\t: GenuineIntel
model name\t: Intel(R) Xeon(R) CPU E5-2640 0 @ 2.50GHz
physical id\t: 1
cpu cores\t: 2

processor\t: 3
vendor_id\t: GenuineIntel
model name\t: Intel(R) Xeon(R) CPU E5-2640 0 @ 2.50GHz
physical id\t: 1
cpu cores\t: 2
";

    #[test]
    fn groups_logical_processors_into_sockets() {
        let cpus = parse_cpuinfo(CPUINFO);
        assert_eq!(cpus.len(), 2);
        for cpu in &cpus {
            assert_eq!(cpu.manufacturer.as_deref(), Some("Intel"));
            assert_eq!(cpu.cores, Some(2));
            assert_eq!(cpu.threads, Some(2));
            assert_eq!(cpu.speed, Some(2500)); // from "@ 2.50GHz", not cpu MHz
            assert!(cpu.name.as_deref().unwrap().contains("E5-2640"));
        }
    }

    #[test]
    fn single_socket_without_physical_id_is_one_cpu() {
        let info = "\
processor\t: 0
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 5
cpu MHz\t\t: 3400.000
cpu cores\t: 6

processor\t: 1
vendor_id\t: AuthenticAMD
model name\t: AMD Ryzen 5
cpu cores\t: 6
";
        let cpus = parse_cpuinfo(info);
        assert_eq!(cpus.len(), 1);
        assert_eq!(cpus[0].manufacturer.as_deref(), Some("AMD"));
        assert_eq!(cpus[0].threads, Some(2));
        assert_eq!(cpus[0].cores, Some(6));
        // No "@ GHz" in the model, so falls back to rounded cpu MHz.
        assert_eq!(cpus[0].speed, Some(3400));
    }

    #[test]
    fn empty_input_yields_no_cpus() {
        assert!(parse_cpuinfo("").is_empty());
    }
}
