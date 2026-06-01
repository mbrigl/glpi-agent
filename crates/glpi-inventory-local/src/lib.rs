// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-inventory-local` — local-system inventory for the GLPI Agent Rust
//! workspace (v2.0.0).
//!
//! Collects the local machine's inventory into a [`Content`] payload, one
//! category at a time. Each category parses a captured data source (a command's
//! output, a `/proc` or `/sys` file) into a typed section; the parsers are pure
//! and unit-tested, while the live collectors are thin platform wrappers.
//!
//! Phase 6 lands the categories incrementally, Linux first. Currently
//! available:
//!
//! - [`categories::os`] — operating-system identity,
//! - [`categories::cpu`] — physical CPUs,
//! - [`categories::memory`] — memory modules,
//! - [`categories::software`] — installed packages,
//! - [`categories::network`] — network interfaces,
//! - [`categories::hardware`] — BIOS / system / board identity,
//! - [`categories::storage`] — disks and optical drives,
//! - [`categories::process`] — running processes,
//! - [`categories::pci`] — PCI controllers,
//! - [`categories::usb`] — USB devices,
//! - [`categories::user`] — logged-in users,
//! - [`categories::battery`] — batteries,
//! - [`categories::environment`] — process environment variables,
//! - [`categories::video`] / [`categories::sound`] — display and audio cards,
//! - [`categories::printer`] — CUPS printers,
//! - [`categories::monitor`] — monitors via EDID.

pub mod categories;
pub mod content;
pub mod task;

pub use categories::{
    env_from_vars, parse_cpuinfo, parse_dmidecode_hardware, parse_dmidecode_memory, parse_edid,
    parse_interfaces, parse_lpstat, parse_lsblk, parse_lspci, parse_lspci_sound, parse_lspci_video,
    parse_lsusb, parse_os_release, parse_packages, parse_power_supply_uevent, parse_ps,
    parse_smartctl_info, parse_timezone_name, parse_who, Battery, Bios, Controller, Cpu, EnvVar,
    Hardware, MemoryModule, Monitor, NetworkInterface, OperatingSystem, Printer, Process,
    SmartInfo, Software, Sound, Storage, Timezone, UsbDevice, User, Video,
};
pub use content::Content;
pub use task::LocalInventory;
