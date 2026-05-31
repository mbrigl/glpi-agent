// SPDX-License-Identifier: GPL-2.0-only

//! Core value types shared across every task crate.
//!
//! These are deliberately protocol-agnostic: they model *what* the agent knows
//! about a device, while the `protocol` module decides *how* it is encoded for
//! a particular GLPI server version.

pub mod device;
pub mod inventory;
pub mod network;
pub mod snmp;

pub use device::{AssetType, Device};
pub use inventory::{InventoryCategory, InventoryResult};
pub use network::{MacAddress, NetworkInterface};
pub use snmp::{AuthProtocol, PrivProtocol, SnmpCredentials, SnmpVersion};
