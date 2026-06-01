// SPDX-License-Identifier: GPL-2.0-only

//! The local-inventory task.
//!
//! [`LocalInventory`] runs the category collectors and assembles their output
//! into a [`Content`], honouring a `no-category` exclusion set (the GLPI
//! category names). The per-category parsers are unit-tested in their modules;
//! this task is the thin orchestration that gathers the live sections and
//! applies the filter.

use std::collections::HashSet;

use crate::categories::{
    battery, cpu, environment, hardware, memory, monitor, network, os, pci, printer, process,
    software, sound, storage, usb, user, video,
};
use crate::content::Content;

/// Runs the local inventory categories and produces the inventory content.
#[derive(Debug, Default, Clone)]
pub struct LocalInventory {
    /// Disabled category names (lower-cased), from `no-category`.
    disabled: HashSet<String>,
}

impl LocalInventory {
    /// Creates a local-inventory task that collects every category.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Excludes the given categories (the GLPI `no-category` names, matched
    /// case-insensitively).
    #[must_use]
    pub fn with_disabled_categories<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.disabled = categories
            .into_iter()
            .map(|c| c.as_ref().to_ascii_lowercase())
            .collect();
        self
    }

    /// Returns `true` if `category` should be collected.
    #[must_use]
    pub fn is_enabled(&self, category: &str) -> bool {
        !self.disabled.contains(category)
    }

    /// Collects the enabled categories into a [`Content`].
    ///
    /// Empty sections (a collector that found nothing, or a disabled category)
    /// are omitted from the result.
    #[must_use]
    pub fn collect(&self) -> Content {
        let mut content = Content {
            version_client: Some(crate::content::VERSION_CLIENT.to_owned()),
            ..Content::default()
        };

        if self.is_enabled("os") {
            let os = os::collect();
            content.operating_system = (os != os::OperatingSystem::default()).then_some(os);
        }

        // dmidecode is read once; the bios and hardware sections gate separately.
        if self.is_enabled("bios") || self.is_enabled("hardware") {
            let (bios, hardware) = hardware::collect();
            if self.is_enabled("bios") {
                content.bios = bios;
            }
            if self.is_enabled("hardware") {
                content.hardware = hardware;
            }
        }

        if self.is_enabled("cpu") {
            content.cpus = cpu::collect();
        }
        if self.is_enabled("memory") {
            content.memories = memory::collect();
        }
        if self.is_enabled("software") {
            content.softwares = software::collect();
        }
        if self.is_enabled("network") {
            content.networks = network::collect();
        }
        if self.is_enabled("storage") {
            content.storages = storage::collect();
        }
        if self.is_enabled("process") {
            content.processes = process::collect();
        }
        if self.is_enabled("controller") {
            content.controllers = pci::collect();
        }
        if self.is_enabled("usb") {
            content.usb_devices = usb::collect();
        }
        if self.is_enabled("user") {
            content.users = user::collect();
        }
        if self.is_enabled("battery") {
            content.batteries = battery::collect();
        }
        if self.is_enabled("environment") {
            content.envs = environment::collect();
        }
        if self.is_enabled("video") {
            content.videos = video::collect();
        }
        if self.is_enabled("sound") {
            content.sounds = sound::collect();
        }
        if self.is_enabled("printer") {
            content.printers = printer::collect();
        }
        if self.is_enabled("monitor") {
            content.monitors = monitor::collect();
        }
        content
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

    #[test]
    fn no_category_excludes_a_section() {
        // The environment is always populated (PATH etc.), so it's a reliable
        // probe for the filter on any platform.
        assert!(!LocalInventory::new().collect().envs.is_empty());

        let filtered = LocalInventory::new()
            .with_disabled_categories(["Environment"]) // case-insensitive
            .collect();
        assert!(filtered.envs.is_empty());
    }

    #[test]
    fn is_enabled_respects_the_disabled_set() {
        let task = LocalInventory::new().with_disabled_categories(["cpu", "memory"]);
        assert!(!task.is_enabled("cpu"));
        assert!(!task.is_enabled("memory"));
        assert!(task.is_enabled("network"));
    }
}
