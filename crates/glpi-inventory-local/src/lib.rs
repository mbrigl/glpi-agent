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
//! - [`categories::pci`] — PCI controllers.

pub mod categories;
pub mod content;
pub mod task;

pub use categories::{
    parse_cpuinfo, parse_dmidecode_hardware, parse_dmidecode_memory, parse_interfaces, parse_lsblk,
    parse_lspci, parse_os_release, parse_packages, parse_ps, parse_timezone_name, Bios, Controller,
    Cpu, Hardware, MemoryModule, NetworkInterface, OperatingSystem, Process, Software, Storage,
    Timezone,
};
pub use content::Content;
pub use task::LocalInventory;
