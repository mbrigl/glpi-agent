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
//! - [`categories::os`] — operating-system identity.

pub mod categories;
pub mod content;

pub use categories::{parse_os_release, OperatingSystem};
pub use content::Content;
