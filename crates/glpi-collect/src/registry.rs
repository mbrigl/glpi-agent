// SPDX-License-Identifier: GPL-2.0-only

//! Windows registry collection (`getFromRegistry`).
//!
//! Registry access is Windows-only, so the read itself sits behind the
//! [`RegistryReader`] seam: the live reader is provided on Windows, while tests
//! (and non-Windows builds) use [`MockRegistry`]. The value model
//! ([`RegistryValue`]) and its GLPI string rendering — notably `REG_MULTI_SZ`,
//! which GLPI joins with new-lines — are cross-platform and unit-tested.

use std::collections::BTreeMap;

use glpi_core::error::{AgentError, Result};

/// A typed registry value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryValue {
    /// `REG_SZ` / `REG_EXPAND_SZ`.
    String(String),
    /// `REG_MULTI_SZ` — an ordered list of strings.
    MultiString(Vec<String>),
    /// `REG_DWORD`.
    Dword(u32),
    /// `REG_QWORD`.
    Qword(u64),
    /// `REG_BINARY`.
    Binary(Vec<u8>),
}

impl RegistryValue {
    /// Renders the value the way GLPI expects it as a string. `REG_MULTI_SZ`
    /// joins its entries with new-lines; binary is lower-case hex.
    #[must_use]
    pub fn to_glpi_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::MultiString(parts) => parts.join("\n"),
            Self::Dword(v) => v.to_string(),
            Self::Qword(v) => v.to_string(),
            Self::Binary(bytes) => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }
}

/// Reads registry keys. Implemented live on Windows; mocked elsewhere.
pub trait RegistryReader {
    /// Returns the (name, value) pairs under `key` (e.g.
    /// `HKEY_LOCAL_MACHINE/SOFTWARE/Vendor`).
    ///
    /// # Errors
    ///
    /// Returns an error if the key cannot be opened or read.
    fn read_values(&self, key: &str) -> Result<BTreeMap<String, RegistryValue>>;
}

/// An in-memory registry for tests and non-Windows builds.
#[derive(Debug, Default, Clone)]
pub struct MockRegistry {
    keys: BTreeMap<String, BTreeMap<String, RegistryValue>>,
}

impl MockRegistry {
    /// Builds an empty mock registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a value under `key`.
    #[must_use]
    pub fn with_value(mut self, key: &str, name: &str, value: RegistryValue) -> Self {
        self.keys
            .entry(key.to_owned())
            .or_default()
            .insert(name.to_owned(), value);
        self
    }
}

impl RegistryReader for MockRegistry {
    fn read_values(&self, key: &str) -> Result<BTreeMap<String, RegistryValue>> {
        self.keys
            .get(key)
            .cloned()
            .ok_or_else(|| AgentError::Task(format!("registry key not found: {key}")))
    }
}

/// The reader used on non-Windows hosts: registry access is unsupported there.
#[derive(Debug, Default, Clone)]
pub struct UnsupportedRegistry;

impl RegistryReader for UnsupportedRegistry {
    fn read_values(&self, _key: &str) -> Result<BTreeMap<String, RegistryValue>> {
        Err(AgentError::Unsupported(
            "registry collection is only available on Windows".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{MockRegistry, RegistryReader, RegistryValue, UnsupportedRegistry};

    #[test]
    fn multi_sz_joins_with_newlines() {
        let value =
            RegistryValue::MultiString(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        assert_eq!(value.to_glpi_string(), "a\nb\nc");
    }

    #[test]
    fn binary_renders_as_hex() {
        assert_eq!(
            RegistryValue::Binary(vec![0x0a, 0xff]).to_glpi_string(),
            "0aff"
        );
        assert_eq!(RegistryValue::Dword(42).to_glpi_string(), "42");
    }

    #[test]
    fn mock_registry_reads_back_values() {
        let reg = MockRegistry::new().with_value(
            "HKLM/SOFTWARE/Vendor",
            "Version",
            RegistryValue::String("1.2.3".to_owned()),
        );
        let values = reg.read_values("HKLM/SOFTWARE/Vendor").unwrap();
        assert_eq!(values["Version"], RegistryValue::String("1.2.3".to_owned()));
        assert!(reg.read_values("HKLM/missing").is_err());
    }

    #[test]
    fn unsupported_registry_errors() {
        assert!(UnsupportedRegistry.read_values("HKLM/x").is_err());
    }
}
