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
//!   the scanner,
//! - [`traits`] — the [`DiscoveryMethod`] abstraction and its result types,
//! - [`scanner`] — the bounded-concurrency parallel [`Scanner`],
//! - [`methods`] — concrete discovery methods (ARP and ping, with NetBIOS/SNMP
//!   to follow).
//!
//! [`DiscoveryMethod`]: traits::DiscoveryMethod
//! [`Scanner`]: scanner::Scanner

pub mod ip_range;
pub mod methods;
pub mod scanner;
pub mod traits;

pub use ip_range::{Ipv4Range, Ipv4RangeIter};
pub use methods::arp::{ArpMethod, ArpTable};
pub use methods::ping::{EchoRequest, PingMethod};
pub use scanner::{ProgressCallback, ScanProgress, Scanner};
pub use traits::{DiscoveredHost, DiscoveryMethod, Probe};
