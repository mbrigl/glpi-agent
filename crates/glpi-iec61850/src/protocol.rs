// SPDX-License-Identifier: GPL-2.0-only

//! The IEC 61850 / MMS protocol seam.
//!
//! [`IedProtocol`] abstracts the handful of MMS operations the inventory scan
//! needs — the same ones the upstream agent drives through libiec61850
//! (`IedConnection_getServerDirectory`, `…getLogicalDeviceDirectory`,
//! `…getLogicalNodeDirectory`, `…readStringValue`). The scan logic in
//! [`crate::device`] is written against this trait, so it is fully unit-tested
//! with the in-memory [`MockProtocol`](crate::mock::MockProtocol); a real
//! backend (the libiec61850 FFI behind the `libiec61850` feature, or a
//! pure-Rust MMS client) implements the same trait.

use async_trait::async_trait;
use glpi_core::error::Result;

/// IEC 61850 functional constraint, restricting which attributes a read sees.
/// The physical-nameplate values the scan reads are description constants
/// ([`FunctionalConstraint::DC`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionalConstraint {
    /// `DC` — description / configuration constants (e.g. `PhyNam`).
    DC,
}

/// The MMS operations needed to inventory an IED.
///
/// Every method maps to a libiec61850 client call; directory listings return
/// the child object names at that level (logical devices, logical nodes, data
/// objects), and [`read_string`](Self::read_string) reads a named attribute as
/// text.
#[async_trait]
pub trait IedProtocol: Send {
    /// Lists the server's logical-device names
    /// (`IedConnection_getServerDirectory`).
    ///
    /// # Errors
    ///
    /// Propagates a transport/protocol failure.
    async fn server_directory(&mut self) -> Result<Vec<String>>;

    /// Lists a logical device's logical-node names
    /// (`IedConnection_getLogicalDeviceDirectory`).
    ///
    /// # Errors
    ///
    /// Propagates a transport/protocol failure.
    async fn logical_device_directory(&mut self, device: &str) -> Result<Vec<String>>;

    /// Lists a logical node's data-object names
    /// (`IedConnection_getLogicalNodeDirectory`, `ACSI_CLASS_DATA_OBJECT`).
    ///
    /// # Errors
    ///
    /// Propagates a transport/protocol failure.
    async fn logical_node_directory(&mut self, logical_node: &str) -> Result<Vec<String>>;

    /// Reads a named attribute as a string under `fc`
    /// (`IedConnection_readStringValue`); `None` if the value is unset/empty.
    ///
    /// # Errors
    ///
    /// Propagates a transport/protocol failure (a per-reference error is mapped
    /// to `Ok(None)` by callers that skip missing values).
    async fn read_string(
        &mut self,
        reference: &str,
        fc: FunctionalConstraint,
    ) -> Result<Option<String>>;
}
