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

/// Decodes a UTF-16LE byte buffer (as the registry stores `REG_SZ`) into a
/// `String`, dropping a trailing NUL terminator. Pure and unit-tested.
#[must_use]
pub fn decode_utf16le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Decodes a `REG_MULTI_SZ` byte buffer (NUL-separated, double-NUL-terminated
/// UTF-16LE strings) into its entries. Pure and unit-tested.
#[must_use]
pub fn decode_multi_sz(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    units
        .split(|&u| u == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// The live Windows registry reader (backed by `winreg`).
///
/// Reads the named key's values from the 64-bit view, mapping each registry
/// type to a [`RegistryValue`] (decoding `REG_SZ`/`REG_MULTI_SZ` from UTF-16LE).
#[cfg(windows)]
#[derive(Debug, Default, Clone)]
pub struct WindowsRegistry;

#[cfg(windows)]
impl RegistryReader for WindowsRegistry {
    fn read_values(&self, key: &str) -> Result<BTreeMap<String, RegistryValue>> {
        use winreg::enums::{RegType, KEY_READ, KEY_WOW64_64KEY};
        use winreg::RegKey;

        let (hive, subkey) = split_key(key)?;
        let root = RegKey::predef(hive);
        let opened = root
            .open_subkey_with_flags(subkey, KEY_READ | KEY_WOW64_64KEY)
            .map_err(|e| AgentError::Task(format!("opening registry key {key}: {e}")))?;

        let mut values = BTreeMap::new();
        for entry in opened.enum_values() {
            let (name, value) =
                entry.map_err(|e| AgentError::Task(format!("reading registry value: {e}")))?;
            let bytes = &value.bytes;
            let decoded = match value.vtype {
                RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
                    RegistryValue::String(decode_utf16le(bytes))
                }
                RegType::REG_MULTI_SZ => RegistryValue::MultiString(decode_multi_sz(bytes)),
                RegType::REG_DWORD | RegType::REG_DWORD_BIG_ENDIAN => {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[..bytes.len().min(4)]);
                    RegistryValue::Dword(u32::from_le_bytes(buf))
                }
                RegType::REG_QWORD => {
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&bytes[..bytes.len().min(8)]);
                    RegistryValue::Qword(u64::from_le_bytes(buf))
                }
                _ => RegistryValue::Binary(bytes.clone()),
            };
            values.insert(name, decoded);
        }
        Ok(values)
    }
}

/// Splits a key path (`HKLM\SOFTWARE\…` or `HKLM/SOFTWARE/…`) into the predefined
/// hive handle and the back-slash sub-key path.
#[cfg(windows)]
fn split_key(key: &str) -> Result<(isize, String)> {
    use winreg::enums::{
        HKEY_CLASSES_ROOT, HKEY_CURRENT_CONFIG, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, HKEY_USERS,
    };
    let normalized = key.replace('/', "\\");
    let (hive, sub) = normalized
        .split_once('\\')
        .ok_or_else(|| AgentError::Task(format!("registry key has no sub-path: {key}")))?;
    let handle = match hive.to_ascii_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
        "HKU" | "HKEY_USERS" => HKEY_USERS,
        "HKCC" | "HKEY_CURRENT_CONFIG" => HKEY_CURRENT_CONFIG,
        other => return Err(AgentError::Task(format!("unknown registry hive: {other}"))),
    };
    Ok((handle, sub.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{
        decode_multi_sz, decode_utf16le, MockRegistry, RegistryReader, RegistryValue,
        UnsupportedRegistry,
    };

    /// Encodes a `&str` as UTF-16LE bytes with a trailing NUL (as `REG_SZ`).
    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[test]
    fn decodes_utf16le_reg_sz() {
        assert_eq!(decode_utf16le(&utf16le("Hello")), "Hello");
        assert_eq!(decode_utf16le(&[]), "");
    }

    #[test]
    fn decodes_reg_multi_sz() {
        // "a\0b\0\0" in UTF-16LE.
        let mut bytes = utf16le("a");
        bytes.extend(utf16le("b"));
        bytes.extend([0, 0]); // final terminator
        assert_eq!(
            decode_multi_sz(&bytes),
            vec!["a".to_owned(), "b".to_owned()]
        );
    }

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
