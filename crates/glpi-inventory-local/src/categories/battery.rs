// SPDX-License-Identifier: GPL-2.0-only

//! Battery inventory category (Linux `/sys/class/power_supply`).
//!
//! Each power-supply entry of type `Battery` exposes a `uevent` file of
//! `POWER_SUPPLY_*=value` lines; [`parse_power_supply_uevent`] turns one into a
//! [`Battery`] (name, manufacturer, serial, chemistry, voltage in mV, capacity
//! in mWh). Mains adapters are ignored. The parser is pure and unit-tested; the
//! live collector scans the sysfs directory.

use serde::Serialize;

/// A battery / power supply of type `Battery`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Battery {
    /// Battery name (e.g. "BAT0").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Manufacturer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// Chemistry / technology (e.g. "Li-ion").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chemistry: Option<String>,
    /// Design voltage in mV.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage: Option<u64>,
    /// Design capacity in mWh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u64>,
}

/// Parses a `power_supply` `uevent` file, returning a [`Battery`] only when the
/// entry is of type `Battery`.
#[must_use]
pub fn parse_power_supply_uevent(text: &str) -> Option<Battery> {
    let map = uevent_map(text);
    if map.get("POWER_SUPPLY_TYPE").map(String::as_str) != Some("Battery") {
        return None;
    }
    Some(Battery {
        name: map.get("POWER_SUPPLY_NAME").cloned(),
        manufacturer: map.get("POWER_SUPPLY_MANUFACTURER").cloned(),
        serial: map.get("POWER_SUPPLY_SERIAL_NUMBER").cloned(),
        chemistry: map.get("POWER_SUPPLY_TECHNOLOGY").cloned(),
        // sysfs reports micro-volts / micro-watt-hours.
        voltage: micro_to_milli(map.get("POWER_SUPPLY_VOLTAGE_MIN_DESIGN")),
        capacity: micro_to_milli(map.get("POWER_SUPPLY_ENERGY_FULL_DESIGN")),
    })
}

/// Collects the live batteries from sysfs (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Battery> {
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
        return Vec::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("uevent")).ok())
        .filter_map(|text| parse_power_supply_uevent(&text))
        .collect()
}

/// Collects the live batteries (Windows) from `Win32_Battery`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Battery> {
    crate::sys::powershell(
        "Get-CimInstance Win32_Battery | \
         Select-Object Name,DesignVoltage,DesignCapacity,Chemistry | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_batteries(&json))
    .unwrap_or_default()
}

/// Collects the live batteries (macOS) from `ioreg`'s `AppleSmartBattery`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Battery> {
    crate::sys::output("ioreg", &["-r", "-c", "AppleSmartBattery"])
        .map(|text| parse_ioreg_battery(&text))
        .unwrap_or_default()
}

/// Collects the live batteries (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Battery> {
    Vec::new()
}

/// Parses `ioreg -r -c AppleSmartBattery` (macOS) into the battery.
///
/// Reads the `"Key" = value` property lines: `DesignCapacity` (mAh) and
/// `Voltage` (mV) give the design capacity in mWh; `Serial`, `Manufacturer` and
/// `DeviceName` give the identity.
#[must_use]
pub fn parse_ioreg_battery(text: &str) -> Vec<Battery> {
    let mut props = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let Some(key_end) = rest.find('"') else {
            continue;
        };
        let key = &rest[..key_end];
        let Some(eq) = rest[key_end + 1..].find('=') else {
            continue;
        };
        let value = rest[key_end + 1 + eq + 1..].trim().trim_matches('"');
        props
            .entry(key.to_owned())
            .or_insert_with(|| value.to_owned());
    }
    let num = |key: &str| props.get(key).and_then(|v| v.parse::<u64>().ok());
    let text_field = |key: &str| props.get(key).filter(|v| !v.is_empty()).cloned();

    let voltage = num("Voltage");
    let capacity = match (num("DesignCapacity"), voltage) {
        // mAh × mV / 1000 = mWh.
        (Some(mah), Some(mv)) => Some(mah * mv / 1000),
        _ => None,
    };
    let battery = Battery {
        name: text_field("DeviceName"),
        manufacturer: text_field("Manufacturer"),
        serial: text_field("Serial").or_else(|| text_field("BatterySerialNumber")),
        chemistry: None,
        voltage,
        capacity,
    };
    if battery == Battery::default() {
        Vec::new()
    } else {
        vec![battery]
    }
}

/// Parses a `Win32_Battery` `ConvertTo-Json` result into the batteries.
#[must_use]
pub fn parse_win_batteries(json: &str) -> Vec<Battery> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .map(|item| Battery {
            name: crate::jsonutil::str_field(item, "Name"),
            manufacturer: None,
            serial: None,
            chemistry: crate::jsonutil::u64_field(item, "Chemistry").and_then(battery_chemistry),
            voltage: crate::jsonutil::u64_field(item, "DesignVoltage"),
            capacity: crate::jsonutil::u64_field(item, "DesignCapacity"),
        })
        .filter(|b| b != &Battery::default())
        .collect()
}

/// Maps a `Win32_Battery.Chemistry` code to a name.
fn battery_chemistry(code: u64) -> Option<String> {
    let name = match code {
        3 => "Lead Acid",
        4 => "Nickel Cadmium",
        5 => "Nickel Metal Hydride",
        6 => "Lithium-ion",
        7 => "Zinc air",
        8 => "Lithium Polymer",
        _ => return None,
    };
    Some(name.to_owned())
}

/// Parses `KEY=VALUE` uevent lines into a map (empty values dropped).
fn uevent_map(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

/// Converts a micro-unit string value to milli-units.
fn micro_to_milli(value: Option<&String>) -> Option<u64> {
    value.and_then(|v| v.parse::<u64>().ok()).map(|n| n / 1000)
}

#[cfg(test)]
mod tests {
    use super::parse_power_supply_uevent;

    const BAT0: &str = "\
POWER_SUPPLY_NAME=BAT0
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_MANUFACTURER=Sony
POWER_SUPPLY_MODEL_NAME=DELL ABC123
POWER_SUPPLY_SERIAL_NUMBER=12345
POWER_SUPPLY_TECHNOLOGY=Li-ion
POWER_SUPPLY_VOLTAGE_MIN_DESIGN=11400000
POWER_SUPPLY_ENERGY_FULL_DESIGN=60000000
";

    const MAINS: &str = "POWER_SUPPLY_NAME=AC\nPOWER_SUPPLY_TYPE=Mains\nPOWER_SUPPLY_ONLINE=1\n";

    #[test]
    fn parses_a_battery() {
        let battery = parse_power_supply_uevent(BAT0).unwrap();
        assert_eq!(battery.name.as_deref(), Some("BAT0"));
        assert_eq!(battery.manufacturer.as_deref(), Some("Sony"));
        assert_eq!(battery.serial.as_deref(), Some("12345"));
        assert_eq!(battery.chemistry.as_deref(), Some("Li-ion"));
        assert_eq!(battery.voltage, Some(11_400)); // mV
        assert_eq!(battery.capacity, Some(60_000)); // mWh
    }

    #[test]
    fn ignores_mains_adapters() {
        assert_eq!(parse_power_supply_uevent(MAINS), None);
    }

    #[test]
    fn parses_windows_battery_json() {
        use super::parse_win_batteries;
        let json = r#"[{"Name":"DELL ABC123","DesignVoltage":11400,"DesignCapacity":60000,"Chemistry":6}]"#;
        let batteries = parse_win_batteries(json);
        assert_eq!(batteries.len(), 1);
        let b = &batteries[0];
        assert_eq!(b.name.as_deref(), Some("DELL ABC123"));
        assert_eq!(b.voltage, Some(11_400));
        assert_eq!(b.capacity, Some(60_000));
        assert_eq!(b.chemistry.as_deref(), Some("Lithium-ion"));
        assert!(parse_win_batteries("bad").is_empty());
    }

    #[test]
    fn parses_macos_ioreg_battery() {
        use super::parse_ioreg_battery;
        let text = "\
+-o AppleSmartBattery  <class AppleSmartBattery>
    {
      \"DesignCapacity\" = 8694
      \"Voltage\" = 12600
      \"Serial\" = \"F5K123\"
      \"Manufacturer\" = \"SMP\"
      \"DeviceName\" = \"bq40z651\"
    }
";
        let batteries = parse_ioreg_battery(text);
        assert_eq!(batteries.len(), 1);
        let b = &batteries[0];
        assert_eq!(b.name.as_deref(), Some("bq40z651"));
        assert_eq!(b.serial.as_deref(), Some("F5K123"));
        assert_eq!(b.manufacturer.as_deref(), Some("SMP"));
        assert_eq!(b.voltage, Some(12_600));
        assert_eq!(b.capacity, Some(8694 * 12_600 / 1000)); // mAh × mV / 1000 = mWh
        assert!(parse_ioreg_battery("no battery here").is_empty());
    }
}
