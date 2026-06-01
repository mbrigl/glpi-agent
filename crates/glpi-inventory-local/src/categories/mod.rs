// SPDX-License-Identifier: GPL-2.0-only

//! Inventory categories.
//!
//! Each category turns a captured data source (a command's output, a `/proc`
//! or `/sys` file) into a typed payload that becomes part of the inventory
//! [`Content`](crate::content::Content). The parsers are pure and unit-tested
//! against fixtures; the live collectors are thin platform wrappers.
//!
//! Currently available:
//!
//! - [`os`] — operating-system identity (`/etc/os-release` + kernel),
//! - [`cpu`] — physical CPUs (`/proc/cpuinfo`),
//! - [`memory`] — memory modules (`dmidecode -t 17`),
//! - [`software`] — installed packages (dpkg / rpm),
//! - [`network`] — network interfaces (`ip -o link` / `ip -o addr`),
//! - [`hardware`] — BIOS / system / board identity (`dmidecode`).

pub mod cpu;
pub mod hardware;
pub mod memory;
pub mod network;
pub mod os;
pub mod software;

pub(crate) mod dmi;

pub use cpu::{parse_cpuinfo, Cpu};
pub use hardware::{parse_dmidecode_hardware, Bios, Hardware};
pub use memory::{parse_dmidecode_memory, MemoryModule};
pub use network::{parse_interfaces, NetworkInterface};
pub use os::{parse_os_release, OperatingSystem};
pub use software::{parse_packages, Software};
