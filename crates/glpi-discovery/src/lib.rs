// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-discovery` — network discovery and SNMP inventory.
//!
//! Part of the GLPI Agent Rust workspace (v2.0.0). This crate hosts the
//! NetDiscovery / NetInventory tasks (Phase 2+): the parallel scanner, the
//! discovery methods (ping, ARP, NetBIOS, SNMP) and the SNMP stack with its
//! MIB-support modules.
//!
//! Landing incrementally; currently available:
//!
//! - [`ip_range`] — IPv4 range expansion (single / CIDR / `start-end`) feeding
//!   the scanner.

pub mod ip_range;

pub use ip_range::{Ipv4Range, Ipv4RangeIter};
