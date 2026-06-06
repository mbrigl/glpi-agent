// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-wakeonlan` — the Wake-on-LAN task (Phase 9).
//!
//! Part of the GLPI Agent Rust workspace.
//!
//! Builds the 102-byte Wake-on-LAN magic packet ([`MagicPacket`]) and broadcasts
//! it for one or more target MAC addresses ([`WakeOnLanTask`]) over UDP.
//!
//! # Example
//!
//! ```no_run
//! use glpi_core::types::network::MacAddress;
//! use glpi_wakeonlan::WakeOnLanTask;
//!
//! let mac: MacAddress = "de:ad:be:ef:00:01".parse().unwrap();
//! let sent = WakeOnLanTask::new(vec![mac]).wake().unwrap();
//! assert!(sent > 0);
//! ```

pub mod magic_packet;
pub mod task;

pub use magic_packet::{MagicPacket, MAGIC_PACKET_LEN};
pub use task::{WakeOnLanTask, DEFAULT_PORTS};
