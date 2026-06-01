// SPDX-License-Identifier: GPL-2.0-only

//! Shared `dmidecode` output parsing.
//!
//! `dmidecode` prints one blank-line-separated block per SMBIOS structure: a
//! `Handle …` line, the structure name (e.g. "Memory Device"), then indented
//! `Key: Value` fields. [`parse_blocks`] turns that into [`DmiBlock`]s the
//! category parsers (memory, hardware/BIOS) consume; [`clean`] maps the common
//! SMBIOS placeholder values to `None`.

use std::collections::HashMap;

/// One parsed `dmidecode` structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DmiBlock {
    /// The structure name line (e.g. "System Information").
    pub name: String,
    /// The indented `Key: Value` fields.
    pub fields: HashMap<String, String>,
}

impl DmiBlock {
    /// Returns the raw value of `key`, if present.
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Parses full `dmidecode` output into its structures.
pub(crate) fn parse_blocks(text: &str) -> Vec<DmiBlock> {
    text.split("\n\n")
        .filter_map(|block| {
            let mut name: Option<String> = None;
            let mut fields = HashMap::new();
            for line in block.lines() {
                if line.starts_with('\t') || line.starts_with(' ') {
                    if let Some((key, value)) = line.split_once(':') {
                        fields.insert(key.trim().to_owned(), value.trim().to_owned());
                    }
                } else if name.is_none()
                    && !line.starts_with("Handle")
                    && !line.starts_with('#')
                    && !line.trim().is_empty()
                {
                    name = Some(line.trim().to_owned());
                }
            }
            name.map(|name| DmiBlock { name, fields })
        })
        .collect()
}

/// Maps SMBIOS placeholder values ("Unknown", "Not Specified", …) to `None`.
pub(crate) fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| {
            !v.is_empty()
                && !matches!(
                    *v,
                    "Unknown"
                        | "Not Specified"
                        | "<OUT OF SPEC>"
                        | "None"
                        | "Not Provided"
                        | "To Be Filled By O.E.M."
                        | "Default string"
                )
        })
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{clean, parse_blocks};

    #[test]
    fn parses_named_blocks_with_fields() {
        let text = "Handle 0x0001, DMI type 1, 27 bytes\nSystem Information\n\tManufacturer: ACME\n\tProduct Name: Box\n\nHandle 0x0002, DMI type 2\nBase Board Information\n\tManufacturer: ACME\n";
        let blocks = parse_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "System Information");
        assert_eq!(blocks[0].get("Manufacturer"), Some("ACME"));
        assert_eq!(blocks[0].get("Product Name"), Some("Box"));
        assert_eq!(blocks[1].name, "Base Board Information");
    }

    #[test]
    fn clean_drops_placeholders() {
        assert_eq!(clean(Some("ACME")).as_deref(), Some("ACME"));
        assert_eq!(clean(Some("Not Specified")), None);
        assert_eq!(clean(Some("To Be Filled By O.E.M.")), None);
        assert_eq!(clean(Some("  ")), None);
        assert_eq!(clean(None), None);
    }
}
