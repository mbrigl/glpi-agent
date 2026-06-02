// SPDX-License-Identifier: GPL-2.0-only

//! WMI collection (`getFromWMI`).
//!
//! Like the registry, WMI is Windows-only and sits behind the [`WmiClient`]
//! seam. A live client runs on a dedicated COM worker thread (Phase 6b); here we
//! define the seam and an in-memory [`MockWmi`] so the Collect dispatch is
//! tested cross-platform.

use std::collections::BTreeMap;

use glpi_core::error::{AgentError, Result};

/// One WMI instance: its selected properties as name → value.
pub type WmiInstance = BTreeMap<String, String>;

/// Queries WMI. Implemented live on Windows; mocked elsewhere.
pub trait WmiClient {
    /// Returns the instances of `class`, each projected to `properties`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn query(&self, class: &str, properties: &[String]) -> Result<Vec<WmiInstance>>;
}

/// An in-memory WMI provider for tests and non-Windows builds.
#[derive(Debug, Default, Clone)]
pub struct MockWmi {
    classes: BTreeMap<String, Vec<WmiInstance>>,
}

impl MockWmi {
    /// Builds an empty mock provider.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one instance of `class`.
    #[must_use]
    pub fn with_instance(mut self, class: &str, instance: WmiInstance) -> Self {
        self.classes
            .entry(class.to_owned())
            .or_default()
            .push(instance);
        self
    }
}

impl WmiClient for MockWmi {
    fn query(&self, class: &str, properties: &[String]) -> Result<Vec<WmiInstance>> {
        let instances = self.classes.get(class).cloned().unwrap_or_default();
        // Project each instance down to the requested properties (empty = all).
        Ok(instances
            .into_iter()
            .map(|instance| {
                if properties.is_empty() {
                    instance
                } else {
                    properties
                        .iter()
                        .filter_map(|p| instance.get(p).map(|v| (p.clone(), v.clone())))
                        .collect()
                }
            })
            .collect())
    }
}

/// The client used on non-Windows hosts: WMI is unsupported there.
#[derive(Debug, Default, Clone)]
pub struct UnsupportedWmi;

impl WmiClient for UnsupportedWmi {
    fn query(&self, _class: &str, _properties: &[String]) -> Result<Vec<WmiInstance>> {
        Err(AgentError::Unsupported(
            "WMI collection is only available on Windows".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{MockWmi, WmiClient, WmiInstance};

    fn instance(pairs: &[(&str, &str)]) -> WmiInstance {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn query_projects_requested_properties() {
        let wmi = MockWmi::new().with_instance(
            "Win32_Service",
            instance(&[("Name", "wuauserv"), ("State", "Running")]),
        );
        let rows = wmi.query("Win32_Service", &["Name".to_owned()]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("Name").map(String::as_str), Some("wuauserv"));
        assert!(!rows[0].contains_key("State"));
    }

    #[test]
    fn unknown_class_yields_no_rows() {
        assert!(MockWmi::new().query("Win32_Nope", &[]).unwrap().is_empty());
    }
}
