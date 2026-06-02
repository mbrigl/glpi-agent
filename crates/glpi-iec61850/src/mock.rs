// SPDX-License-Identifier: GPL-2.0-only

//! An in-memory [`IedProtocol`] for offline tests.
//!
//! [`MockProtocol`] replays a fixed IEC 61850 object tree (logical devices →
//! logical nodes → data objects) and a set of named attribute values, so the
//! scan logic in [`crate::device`] can be exercised end-to-end without a live
//! IED — the "mock IED responses" the migration plan calls for.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::protocol::{FunctionalConstraint, IedProtocol};

/// A configurable in-memory IED.
#[derive(Debug, Default, Clone)]
pub struct MockProtocol {
    /// Logical-device names (server directory).
    devices: Vec<String>,
    /// `device` → logical-node names.
    logical_nodes: BTreeMap<String, Vec<String>>,
    /// `device/LN` → data-object names.
    data_objects: BTreeMap<String, Vec<String>>,
    /// Attribute reference → string value.
    values: BTreeMap<String, String>,
}

impl MockProtocol {
    /// Creates an empty mock (a server with no logical devices).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a logical device with its logical nodes.
    #[must_use]
    pub fn with_logical_device(mut self, device: &str, logical_nodes: &[&str]) -> Self {
        self.devices.push(device.to_owned());
        self.logical_nodes.insert(
            device.to_owned(),
            logical_nodes.iter().map(|s| (*s).to_owned()).collect(),
        );
        self
    }

    /// Adds the data objects of a `device/LN` logical node.
    #[must_use]
    pub fn with_data_objects(mut self, logical_node: &str, objects: &[&str]) -> Self {
        self.data_objects.insert(
            logical_node.to_owned(),
            objects.iter().map(|s| (*s).to_owned()).collect(),
        );
        self
    }

    /// Sets a named attribute's string value.
    #[must_use]
    pub fn with_value(mut self, reference: &str, value: &str) -> Self {
        self.values.insert(reference.to_owned(), value.to_owned());
        self
    }
}

#[async_trait]
impl IedProtocol for MockProtocol {
    async fn server_directory(&mut self) -> Result<Vec<String>> {
        Ok(self.devices.clone())
    }

    async fn logical_device_directory(&mut self, device: &str) -> Result<Vec<String>> {
        Ok(self.logical_nodes.get(device).cloned().unwrap_or_default())
    }

    async fn logical_node_directory(&mut self, logical_node: &str) -> Result<Vec<String>> {
        Ok(self
            .data_objects
            .get(logical_node)
            .cloned()
            .unwrap_or_default())
    }

    async fn read_string(
        &mut self,
        reference: &str,
        _fc: FunctionalConstraint,
    ) -> Result<Option<String>> {
        Ok(self.values.get(reference).cloned())
    }
}
