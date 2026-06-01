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
//! - [`software`] — installed packages (dpkg / rpm).

pub mod cpu;
pub mod memory;
pub mod os;
pub mod software;

pub use cpu::{parse_cpuinfo, Cpu};
pub use memory::{parse_dmidecode_memory, MemoryModule};
pub use os::{parse_os_release, OperatingSystem};
pub use software::{parse_packages, Software};
