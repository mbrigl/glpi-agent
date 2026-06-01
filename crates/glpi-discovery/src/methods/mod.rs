// SPDX-License-Identifier: GPL-2.0-only

//! Discovery methods: the concrete [`DiscoveryMethod`] implementations the
//! scanner runs against each address.
//!
//! Each method is one detection technique. Landing incrementally; currently
//! available:
//!
//! - [`arp`] — MAC resolution from the system ARP cache,
//! - [`ping`] — liveness via unprivileged ICMP echo with a TCP-connect fallback.
//!
//! NetBIOS and SNMP follow in later units.
//!
//! [`DiscoveryMethod`]: crate::traits::DiscoveryMethod

pub mod arp;
pub mod ping;

pub use arp::{ArpMethod, ArpTable};
pub use ping::{EchoRequest, PingMethod};
