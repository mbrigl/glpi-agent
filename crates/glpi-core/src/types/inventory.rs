// SPDX-License-Identifier: GPL-2.0-only

//! Inventory result scaffolding: [`InventoryCategory`] and [`InventoryResult`].
//!
//! The concrete per-category payloads live in `glpi-inventory-local`; this
//! module only provides the category enumeration (used for the `no-category`
//! / `required-category` filters) and a thin result container.

use serde::{Deserialize, Serialize};

use super::device::Device;

/// A single inventory category.
///
/// Names match the keys used in the GLPI native JSON `content` object, so the
/// enum can drive category filtering directly from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InventoryCategory {
    /// BIOS, motherboard, chassis, UUID.
    Hardware,
    /// CPUs, cores, threads, cache.
    Cpu,
    /// RAM modules.
    Memory,
    /// Disks, optical drives, SMART data.
    Storage,
    /// Network interfaces.
    Network,
    /// Operating system, kernel, architecture.
    Os,
    /// Installed software / packages.
    Software,
    /// Running processes.
    Process,
    /// Local and logged-in users.
    User,
    /// Local printers.
    Printer,
    /// Monitors (via EDID).
    Monitor,
    /// USB devices.
    Usb,
    /// Antivirus products.
    Antivirus,
    /// Virtual machines / containers.
    Virtualmachine,
}

impl InventoryCategory {
    /// Every category, in a stable order. Handy for iterating filters in tests
    /// and in the `--list-categories` CLI helper.
    pub const ALL: [Self; 14] = [
        Self::Hardware,
        Self::Cpu,
        Self::Memory,
        Self::Storage,
        Self::Network,
        Self::Os,
        Self::Software,
        Self::Process,
        Self::User,
        Self::Printer,
        Self::Monitor,
        Self::Usb,
        Self::Antivirus,
        Self::Virtualmachine,
    ];
}

/// The outcome of an inventory run, before it is serialized into a protocol
/// message.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryResult {
    /// The device this inventory describes.
    pub device: Device,
    /// Whether this is a partial inventory (a subset of categories).
    pub partial: bool,
    /// Categories that were actually collected in this run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<InventoryCategory>,
}

#[cfg(test)]
mod tests {
    use super::InventoryCategory;

    #[test]
    fn all_is_complete_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in InventoryCategory::ALL {
            assert!(seen.insert(c), "duplicate category in ALL: {c:?}");
        }
        assert_eq!(seen.len(), InventoryCategory::ALL.len());
    }

    #[test]
    fn serializes_snake_case() {
        let json = serde_json::to_string(&InventoryCategory::Virtualmachine).unwrap();
        assert_eq!(json, "\"virtualmachine\"");
    }
}
