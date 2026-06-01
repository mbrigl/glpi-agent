// SPDX-License-Identifier: GPL-2.0-only

//! The local-inventory task.
//!
//! [`LocalInventory`] runs the available category collectors and assembles
//! their output into a [`Content`]. The per-category parsers are unit-tested in
//! their modules; this task is the thin orchestration that gathers the live
//! sections (currently OS and CPU; more categories plug in here).

use crate::categories::{cpu, hardware, memory, network, os, software, storage};
use crate::content::Content;

/// Runs the local inventory categories and produces the inventory content.
#[derive(Debug, Default, Clone)]
pub struct LocalInventory;

impl LocalInventory {
    /// Creates a local-inventory task.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Collects every available category into a [`Content`].
    ///
    /// An empty operating-system section (e.g. on a non-Linux build) is omitted
    /// rather than serialized as an empty object.
    #[must_use]
    pub fn collect(&self) -> Content {
        let operating_system = {
            let os = os::collect();
            (os != os::OperatingSystem::default()).then_some(os)
        };
        let (bios, hardware) = hardware::collect();
        Content {
            bios,
            hardware,
            operating_system,
            cpus: cpu::collect(),
            memories: memory::collect(),
            softwares: software::collect(),
            networks: network::collect(),
            storages: storage::collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LocalInventory;

    #[test]
    fn collect_runs_without_panicking() {
        // Environment-dependent contents, but the task must always assemble a
        // Content without error on any platform.
        let _content = LocalInventory::new().collect();
    }
}
