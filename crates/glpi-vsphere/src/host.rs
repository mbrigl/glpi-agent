// SPDX-License-Identifier: GPL-2.0-only

//! The host "full info" model — the offline-replayable representation of an ESX
//! host and its virtual machines.
//!
//! [`HostInfo`] is the intermediate the SOAP client parses a `HostSystem` (plus
//! its `VirtualMachine` children) into. It is plain `serde` data: the `--dump`
//! mode serializes it to JSON and `--dumpfile` replays it, so the conversion to
//! a GLPI inventory ([`crate::content::EsxContent`]) can be exercised entirely
//! offline. This mirrors the Perl agent's `*-hostfullinfo.dump` fixtures, only
//! in a typed JSON shape rather than a `Data::Dumper` blob.

use serde::{Deserialize, Serialize};

/// A parsed ESX/ESXi host together with the virtual machines it runs.
///
/// Every field is optional because a real vSphere endpoint may omit any of
/// them (permissions, host generation, partial property retrieval). The
/// conversion to inventory drops empty sections accordingly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostInfo {
    /// Host name (DNS host name, else the managed-object name).
    pub name: Option<String>,
    /// Product full name, e.g. `"VMware ESXi 7.0.3 build-19898904"`.
    pub product_full_name: Option<String>,
    /// Product version, e.g. `"7.0.3"`.
    pub product_version: Option<String>,
    /// SMBIOS system UUID (`hardware.systemInfo.uuid`).
    pub uuid: Option<String>,
    /// Hardware vendor / manufacturer (`hardware.systemInfo.vendor`).
    pub vendor: Option<String>,
    /// Hardware model (`hardware.systemInfo.model`).
    pub model: Option<String>,
    /// Service-tag / enclosure serial number, when reported.
    pub serial: Option<String>,
    /// BIOS version (`hardware.biosInfo.biosVersion`).
    pub bios_version: Option<String>,
    /// BIOS release date (`hardware.biosInfo.releaseDate`).
    pub bios_date: Option<String>,
    /// CPU marketing model (`summary.hardware.cpuModel`).
    pub cpu_model: Option<String>,
    /// CPU nominal speed in MHz (`summary.hardware.cpuMhz`).
    pub cpu_mhz: Option<u64>,
    /// Number of physical CPU packages / sockets.
    pub cpu_packages: Option<u32>,
    /// Total number of physical cores across all packages.
    pub cpu_cores: Option<u32>,
    /// Total number of hardware threads across all packages.
    pub cpu_threads: Option<u32>,
    /// Total physical memory in bytes (`hardware.memorySize`).
    pub memory_bytes: Option<u64>,
    /// Host network interfaces (physical + virtual NICs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<HostNic>,
    /// Virtual machines registered on this host.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub virtual_machines: Vec<VmInfo>,
}

/// A host network interface (physical pNIC or management vNIC).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostNic {
    /// Device name, e.g. `"vmnic0"` or `"vmk0"`.
    pub name: Option<String>,
    /// MAC address (`aa:bb:cc:dd:ee:ff`).
    pub mac: Option<String>,
    /// IPv4 address, when the NIC carries one (management interfaces).
    pub ip: Option<String>,
    /// Subnet mask for [`HostNic::ip`].
    pub netmask: Option<String>,
}

/// A virtual machine as reported by the host.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VmInfo {
    /// Display name (`summary.config.name`).
    pub name: Option<String>,
    /// Instance UUID / BIOS UUID (`summary.config.uuid`).
    pub uuid: Option<String>,
    /// Configured virtual CPU count (`summary.config.numCpu`).
    pub vcpu: Option<u32>,
    /// Configured memory in megabytes (`summary.config.memorySizeMB`).
    pub memory_mb: Option<u64>,
    /// Power state as reported by vSphere
    /// (`poweredOn` / `poweredOff` / `suspended`).
    pub power_state: Option<String>,
    /// Guest OS full name (`summary.config.guestFullName` or
    /// `summary.guest.guestFullName`) — reported to GLPI 10.0.17+.
    pub guest_full_name: Option<String>,
    /// Free-form annotation (`summary.config.annotation`).
    pub comment: Option<String>,
    /// Virtual NIC MAC addresses.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mac_addresses: Vec<String>,
    /// Guest IP addresses (VMware tools), primary first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ip_addresses: Vec<String>,
}

/// Placeholder / invalid hardware strings that SMBIOS firmware commonly emits.
///
/// vSphere passes these through verbatim from the host's SMBIOS tables; GLPI
/// treats them as noise, so the inventory conversion drops them (the "BIOS
/// filter" of the migration plan).
const INVALID_VALUES: &[&str] = &[
    "",
    "0",
    "00000000",
    "0.0.0.0",
    "none",
    "n/a",
    "na",
    "unknown",
    "not specified",
    "not available",
    "no asset tag",
    "no asset information",
    "default string",
    "system serial number",
    "system manufacturer",
    "system product name",
    "to be filled by o.e.m.",
    "to be filled by o.e.m",
    "chassis serial number",
    "empty",
    "<bad index>",
    "................",
    "xxxxxxxxxxxxxxxxxx",
];

/// Returns `value` trimmed if it is meaningful, or `None` if it is empty or one
/// of the well-known SMBIOS placeholder strings ([`INVALID_VALUES`]).
///
/// The comparison is case-insensitive and ignores surrounding whitespace.
#[must_use]
pub fn clean(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    let lower = trimmed.to_ascii_lowercase();
    if INVALID_VALUES.contains(&lower.as_str()) {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{clean, HostInfo, VmInfo};

    #[test]
    fn clean_drops_placeholders_case_insensitively() {
        assert_eq!(clean(Some("Dell Inc.")), Some("Dell Inc.".to_owned()));
        assert_eq!(
            clean(Some("  PowerEdge R740 ")),
            Some("PowerEdge R740".to_owned())
        );
        assert_eq!(clean(Some("To Be Filled By O.E.M.")), None);
        assert_eq!(clean(Some("System Serial Number")), None);
        assert_eq!(clean(Some("0.0.0.0")), None);
        assert_eq!(clean(Some("   ")), None);
        assert_eq!(clean(None), None);
    }

    #[test]
    fn host_info_round_trips_through_json() {
        let host = HostInfo {
            name: Some("esx1.lab".to_owned()),
            uuid: Some("564d-uuid".to_owned()),
            memory_bytes: Some(68_719_476_736),
            virtual_machines: vec![VmInfo {
                name: Some("db01".to_owned()),
                vcpu: Some(4),
                memory_mb: Some(8192),
                power_state: Some("poweredOn".to_owned()),
                ..VmInfo::default()
            }],
            ..HostInfo::default()
        };
        let json = serde_json::to_string(&host).unwrap();
        let back: HostInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(host, back);
    }

    #[test]
    fn deserializes_partial_dump_with_defaults() {
        // A dump that only carries a name must still load (every field defaults).
        let host: HostInfo = serde_json::from_str(r#"{"name":"esx2"}"#).unwrap();
        assert_eq!(host.name.as_deref(), Some("esx2"));
        assert!(host.virtual_machines.is_empty());
        assert!(host.memory_bytes.is_none());
    }
}
