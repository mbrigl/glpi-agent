// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-core` — shared types, configuration, protocol, auth and logging for
//! the GLPI Agent Rust workspace.
//!
//! This crate is the foundation every task crate builds on. Phase 1 lands the
//! pieces incrementally; currently available:
//!
//! - [`error`] — the workspace-wide [`AgentError`] / [`Result`] types,
//! - [`types`] — protocol-agnostic value types (device, network, SNMP,
//!   inventory),
//! - [`config`] — the layered options model and its merge machinery,
//! - [`protocol`] — GLPI native JSON messages and category-filter logic,
//! - [`logging`] — the logger facade with stderr / file / callback backends.
//!
//! The `auth` module follows later in Phase 1.

pub mod config;
pub mod error;
pub mod logging;
pub mod protocol;
pub mod types;

pub use error::{AgentError, Result};
