// SPDX-License-Identifier: GPL-2.0-only

//! Discovery methods: the concrete [`DiscoveryMethod`] implementations the
//! scanner runs against each address.
//!
//! Each method is one detection technique. Landing incrementally; currently
//! available:
//!
//! - [`arp`] — MAC resolution from the system ARP cache,
//! - [`ping`] — liveness via unprivileged ICMP echo with a TCP-connect fallback,
//! - [`netbios`] — hostname resolution via a NetBIOS node status query,
//! - [`snmp`] — SNMP host detection across one or more credentials,
//! - [`iec61850`] — IED detection via the MMS port (TCP 102).
//!
//! [`DiscoveryMethod`]: crate::traits::DiscoveryMethod

pub mod arp;
pub mod iec61850;
pub mod netbios;
pub mod ping;
pub mod snmp;

pub use arp::{ArpMethod, ArpTable};
pub use iec61850::{Iec61850Method, MMS_PORT};
pub use netbios::{NetBiosMethod, NetBiosName};
pub use ping::{EchoRequest, PingMethod};
pub use snmp::SnmpMethod;
