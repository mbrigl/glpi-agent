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
//! - [`os`] — operating-system identity (`/etc/os-release` + kernel).

pub mod os;

pub use os::{parse_os_release, OperatingSystem};
