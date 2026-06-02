// SPDX-License-Identifier: GPL-2.0-only

//! The ESX inventory `content` and the [`HostInfo`] → [`EsxContent`] conversion.
//!
//! An ESX inventory is a regular GLPI computer inventory for the hypervisor host
//! that additionally carries a `virtualmachines` section. We reuse the local
//! inventory's category structs ([`Bios`], [`Hardware`], [`Cpu`], …) for the
//! host sections and add a [`VirtualMachine`] type for the guests.
//!
//! The conversion in [`EsxContent::from_host`] is the well-tested core: it
//! applies the BIOS placeholder filter ([`crate::host::clean`]), synthesizes a
//! single memory module from the host's total RAM when no per-DIMM data is
//! available, splits the physical CPU packages, and (for GLPI 10.0.17+) reports
//! each VM's guest OS and IP addresses.

use std::net::IpAddr;
use std::str::FromStr;

use glpi_core::types::network::MacAddress;
use glpi_inventory_local::{Bios, Cpu, Hardware, MemoryModule, NetworkInterface, OperatingSystem};
use serde::Serialize;

use crate::host::{clean, HostInfo, VmInfo};

/// The agent identifier reported in `content.versionclient`.
pub const VERSION_CLIENT: &str = concat!("GLPI-Agent_v", env!("CARGO_PKG_VERSION"));

/// Knobs that influence how a [`HostInfo`] is rendered to inventory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConversionOptions {
    /// Emit per-VM `operatingsystem` and `ipaddress` (GLPI 10.0.17+, schema
    /// v1.1.36). Older servers reject those keys, so they are omitted by
    /// default.
    pub vm_os_ip: bool,
}

/// The assembled ESX inventory content. Empty sections are omitted from the
/// JSON so a host with, say, no readable BIOS info just leaves it out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EsxContent {
    /// Agent identifier (required by the GLPI inventory schema).
    #[serde(rename = "versionclient", skip_serializing_if = "Option::is_none")]
    pub version_client: Option<String>,
    /// BIOS / system identity of the hypervisor host.
    #[serde(rename = "bios", skip_serializing_if = "Option::is_none")]
    pub bios: Option<Bios>,
    /// Host-level identity (name, UUID).
    #[serde(rename = "hardware", skip_serializing_if = "Option::is_none")]
    pub hardware: Option<Hardware>,
    /// Hypervisor operating system (ESXi).
    #[serde(rename = "operatingsystem", skip_serializing_if = "Option::is_none")]
    pub operating_system: Option<OperatingSystem>,
    /// Physical CPUs.
    #[serde(rename = "cpus", skip_serializing_if = "Vec::is_empty")]
    pub cpus: Vec<Cpu>,
    /// Memory modules (synthesized from total RAM when no DIMM data exists).
    #[serde(rename = "memories", skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<MemoryModule>,
    /// Host network interfaces.
    #[serde(rename = "networks", skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<NetworkInterface>,
    /// Virtual machines hosted on this hypervisor.
    #[serde(rename = "virtualmachines", skip_serializing_if = "Vec::is_empty")]
    pub virtual_machines: Vec<VirtualMachine>,
}

/// A guest virtual machine, serialized to the GLPI `virtualmachines` schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct VirtualMachine {
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Instance / BIOS UUID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// GLPI power status (`running` / `off` / `paused` / `crashed`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Configured memory in megabytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    /// Configured virtual CPU count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcpu: Option<u32>,
    /// Hypervisor family — always `"VMware"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vmtype: Option<String>,
    /// Hypervisor product — `"VMware ESX"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsystem: Option<String>,
    /// Free-form annotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// MAC addresses, comma-joined (GLPI stores a single string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    /// Guest OS full name (GLPI 10.0.17+).
    #[serde(rename = "operatingsystem", skip_serializing_if = "Option::is_none")]
    pub operating_system: Option<String>,
    /// Primary guest IP address (GLPI 10.0.17+).
    #[serde(rename = "ipaddress", skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
}

impl EsxContent {
    /// Builds the inventory content for one host.
    #[must_use]
    pub fn from_host(host: &HostInfo, options: ConversionOptions) -> Self {
        Self {
            version_client: Some(VERSION_CLIENT.to_owned()),
            bios: bios_section(host),
            hardware: hardware_section(host),
            operating_system: os_section(host),
            cpus: cpu_section(host),
            memories: memory_section(host),
            networks: network_section(host),
            virtual_machines: host
                .virtual_machines
                .iter()
                .map(|vm| VirtualMachine::from_vm(vm, options))
                .collect(),
        }
    }
}

/// Builds the BIOS section, dropping placeholder firmware values. Returns
/// `None` when nothing meaningful survives the filter.
fn bios_section(host: &HostInfo) -> Option<Bios> {
    let bios = Bios {
        bios_version: clean(host.bios_version.as_deref()),
        bios_date: clean(host.bios_date.as_deref()),
        system_manufacturer: clean(host.vendor.as_deref()),
        system_model: clean(host.model.as_deref()),
        system_serial: clean(host.serial.as_deref()),
        ..Bios::default()
    };
    (bios != Bios::default()).then_some(bios)
}

/// Builds the host identity section.
fn hardware_section(host: &HostInfo) -> Option<Hardware> {
    let hardware = Hardware {
        name: clean(host.name.as_deref()),
        uuid: clean(host.uuid.as_deref()),
        vm_system: None,
    };
    (hardware != Hardware::default()).then_some(hardware)
}

/// Builds the hypervisor OS section from the product strings.
fn os_section(host: &HostInfo) -> Option<OperatingSystem> {
    let full_name = clean(host.product_full_name.as_deref());
    let version = clean(host.product_version.as_deref());
    if full_name.is_none() && version.is_none() {
        return None;
    }
    Some(OperatingSystem {
        // The hypervisor product name (e.g. "VMware ESXi"); fall back to the
        // generic family when only a version is known.
        name: full_name
            .as_deref()
            .map(product_name)
            .or(Some("VMware ESXi".to_owned())),
        version,
        full_name,
        kernel_name: Some("VMkernel".to_owned()),
        ..OperatingSystem::default()
    })
}

/// Extracts the product name (everything up to the version/build) from a full
/// product string such as `"VMware ESXi 7.0.3 build-19898904"`.
fn product_name(full: &str) -> String {
    let name: String = full
        .split_whitespace()
        .take_while(|tok| !tok.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        full.to_owned()
    } else {
        name
    }
}

/// Builds one [`Cpu`] per physical package, splitting the host's total core and
/// thread counts evenly across the packages.
fn cpu_section(host: &HostInfo) -> Vec<Cpu> {
    let packages = host.cpu_packages.unwrap_or(0);
    if packages == 0 {
        // No package count: emit a single socket if we know anything about it.
        if host.cpu_model.is_none() && host.cpu_mhz.is_none() {
            return Vec::new();
        }
        return vec![Cpu {
            name: clean(host.cpu_model.as_deref()),
            manufacturer: None,
            speed: host.cpu_mhz,
            cores: host.cpu_cores,
            threads: host.cpu_threads,
        }];
    }

    let cores = host.cpu_cores.map(|c| c / packages);
    let threads = host.cpu_threads.map(|t| t / packages);
    (0..packages)
        .map(|_| Cpu {
            name: clean(host.cpu_model.as_deref()),
            manufacturer: None,
            speed: host.cpu_mhz,
            cores,
            threads,
        })
        .collect()
}

/// Builds the memory section. vSphere reports only the host's total RAM, so we
/// synthesize a single "System Memory" module from it (the total-RAM estimate).
fn memory_section(host: &HostInfo) -> Vec<MemoryModule> {
    match host.memory_bytes {
        Some(bytes) if bytes > 0 => vec![MemoryModule {
            capacity: Some(bytes / (1024 * 1024)),
            caption: Some("System Memory".to_owned()),
            ..MemoryModule::default()
        }],
        _ => Vec::new(),
    }
}

/// Builds the host network-interface section, skipping NICs we cannot name.
fn network_section(host: &HostInfo) -> Vec<NetworkInterface> {
    host.interfaces
        .iter()
        .filter_map(|nic| {
            let name = clean(nic.name.as_deref())?;
            let ips = nic
                .ip
                .as_deref()
                .and_then(|ip| IpAddr::from_str(ip.trim()).ok())
                .into_iter()
                .collect();
            Some(NetworkInterface {
                name,
                mac: nic.mac.as_deref().and_then(parse_mac),
                ips,
                mtu: None,
                status: None,
                speed: None,
            })
        })
        .collect()
}

/// Parses a `aa:bb:cc:dd:ee:ff` MAC string, ignoring blanks.
fn parse_mac(mac: &str) -> Option<MacAddress> {
    let mac = mac.trim();
    if mac.is_empty() {
        None
    } else {
        mac.parse::<MacAddress>().ok()
    }
}

impl VirtualMachine {
    /// Converts a parsed [`VmInfo`] into its GLPI inventory shape.
    #[must_use]
    pub fn from_vm(vm: &VmInfo, options: ConversionOptions) -> Self {
        let macs: Vec<&str> = vm
            .mac_addresses
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .collect();
        Self {
            name: clean(vm.name.as_deref()),
            uuid: clean(vm.uuid.as_deref()),
            status: vm.power_state.as_deref().map(power_status),
            memory: vm.memory_mb,
            vcpu: vm.vcpu,
            vmtype: Some("VMware".to_owned()),
            subsystem: Some("VMware ESX".to_owned()),
            comment: clean(vm.comment.as_deref()),
            mac: (!macs.is_empty()).then(|| macs.join(", ")),
            operating_system: options
                .vm_os_ip
                .then(|| clean(vm.guest_full_name.as_deref()))
                .flatten(),
            ip_address: options
                .vm_os_ip
                .then(|| vm.ip_addresses.first().cloned())
                .flatten(),
        }
    }
}

/// Maps a vSphere `powerState` to the GLPI virtual-machine status vocabulary.
fn power_status(state: &str) -> String {
    match state {
        "poweredOn" => "running",
        "poweredOff" => "off",
        "suspended" => "paused",
        other => other,
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{ConversionOptions, EsxContent, VirtualMachine};
    use crate::host::{HostInfo, HostNic, VmInfo};

    fn sample_host() -> HostInfo {
        HostInfo {
            name: Some("esx1.lab".to_owned()),
            product_full_name: Some("VMware ESXi 7.0.3 build-19898904".to_owned()),
            product_version: Some("7.0.3".to_owned()),
            uuid: Some("564dabc".to_owned()),
            vendor: Some("Dell Inc.".to_owned()),
            model: Some("PowerEdge R740".to_owned()),
            serial: Some("To Be Filled By O.E.M.".to_owned()),
            bios_version: Some("2.10.2".to_owned()),
            bios_date: Some("06/15/2021".to_owned()),
            cpu_model: Some("Intel(R) Xeon(R) Gold 6230".to_owned()),
            cpu_mhz: Some(2100),
            cpu_packages: Some(2),
            cpu_cores: Some(40),
            cpu_threads: Some(80),
            memory_bytes: Some(68_719_476_736),
            interfaces: vec![HostNic {
                name: Some("vmk0".to_owned()),
                mac: Some("00:11:22:33:44:55".to_owned()),
                ip: Some("10.0.0.10".to_owned()),
                netmask: Some("255.255.255.0".to_owned()),
            }],
            virtual_machines: vec![VmInfo {
                name: Some("db01".to_owned()),
                uuid: Some("420d-vm".to_owned()),
                vcpu: Some(4),
                memory_mb: Some(8192),
                power_state: Some("poweredOn".to_owned()),
                guest_full_name: Some("Ubuntu Linux (64-bit)".to_owned()),
                comment: Some("primary db".to_owned()),
                mac_addresses: vec!["00:50:56:aa:bb:cc".to_owned()],
                ip_addresses: vec!["10.0.0.20".to_owned()],
            }],
        }
    }

    #[test]
    fn builds_host_sections() {
        let content = EsxContent::from_host(&sample_host(), ConversionOptions::default());

        let bios = content.bios.unwrap();
        assert_eq!(bios.system_manufacturer.as_deref(), Some("Dell Inc."));
        assert_eq!(bios.bios_version.as_deref(), Some("2.10.2"));
        // The placeholder serial is filtered out.
        assert_eq!(bios.system_serial, None);

        let os = content.operating_system.unwrap();
        assert_eq!(os.name.as_deref(), Some("VMware ESXi"));
        assert_eq!(os.version.as_deref(), Some("7.0.3"));

        assert_eq!(content.hardware.unwrap().name.as_deref(), Some("esx1.lab"));
    }

    #[test]
    fn splits_cpu_packages_and_synthesizes_memory() {
        let content = EsxContent::from_host(&sample_host(), ConversionOptions::default());
        assert_eq!(content.cpus.len(), 2);
        assert_eq!(content.cpus[0].cores, Some(20));
        assert_eq!(content.cpus[0].threads, Some(40));
        assert_eq!(content.cpus[0].speed, Some(2100));

        assert_eq!(content.memories.len(), 1);
        assert_eq!(content.memories[0].capacity, Some(65536));
        assert_eq!(
            content.memories[0].caption.as_deref(),
            Some("System Memory")
        );
    }

    #[test]
    fn vm_os_and_ip_gated_on_options() {
        let host = sample_host();
        // Default: no VM OS / IP keys.
        let without = EsxContent::from_host(&host, ConversionOptions::default());
        assert_eq!(without.virtual_machines[0].operating_system, None);
        assert_eq!(without.virtual_machines[0].ip_address, None);

        // GLPI 10.0.17+: VM OS / IP reported.
        let with = EsxContent::from_host(&host, ConversionOptions { vm_os_ip: true });
        let vm = &with.virtual_machines[0];
        assert_eq!(vm.status.as_deref(), Some("running"));
        assert_eq!(vm.memory, Some(8192));
        assert_eq!(vm.vmtype.as_deref(), Some("VMware"));
        assert_eq!(vm.mac.as_deref(), Some("00:50:56:aa:bb:cc"));
        assert_eq!(
            vm.operating_system.as_deref(),
            Some("Ubuntu Linux (64-bit)")
        );
        assert_eq!(vm.ip_address.as_deref(), Some("10.0.0.20"));
    }

    #[test]
    fn power_state_maps_to_glpi_vocabulary() {
        let vm = VmInfo {
            power_state: Some("poweredOff".to_owned()),
            ..VmInfo::default()
        };
        let out = VirtualMachine::from_vm(&vm, ConversionOptions::default());
        assert_eq!(out.status.as_deref(), Some("off"));
    }

    #[test]
    fn empty_host_yields_only_version_client() {
        let content = EsxContent::from_host(&HostInfo::default(), ConversionOptions::default());
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 1);
        assert!(json.get("versionclient").is_some());
    }
}
