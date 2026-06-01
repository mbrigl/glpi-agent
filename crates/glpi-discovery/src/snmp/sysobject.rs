// SPDX-License-Identifier: GPL-2.0-only

//! `sysobject.ids` database: classify a device from its `sysObjectID`.
//!
//! The upstream agent ships a `sysobject.ids` file mapping enterprise OIDs to a
//! device's manufacturer, type and model. This module parses that file and
//! reproduces the agent's lookup exactly (see `GLPI::Agent::SNMP::Hardware`):
//!
//! * **Format** — tab-separated `id ⇥ manufacturer ⇥ type ⇥ model ⇥ module`
//!   (type/model/module optional); `#` comments and blank lines are skipped.
//!   Entries are keyed by the `id` field, which is the *enterprise-relative*
//!   OID (the arcs after `1.3.6.1.4.1`), e.g. `9`, `9.1.3`.
//! * **Match** — the device's `sysObjectID` is stripped of its enterprise
//!   prefix (numeric `1.3.6.1.4.1.`, with or without a leading dot; the textual
//!   `iso.3.6.1.4.1.`; or `SNMPv2-SMI::enterprises.`), split into a
//!   manufacturer id and a device id, then resolved by trying the full
//!   `manufacturer.device` key, then progressively shorter device-id prefixes,
//!   then the manufacturer id alone.
//!
//! A consequence of keying on the relative form: any file line written as a
//! *full* OID (a handful exist upstream, e.g. `1.3.6.1.4.1.28507`) is stored
//! verbatim and is effectively unreachable by this lookup — matching the
//! upstream behaviour rather than working around it.

use std::collections::HashMap;

/// One classification entry from `sysobject.ids`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SysObjectEntry {
    /// Manufacturer / vendor name.
    pub manufacturer: Option<String>,
    /// Device type (`NETWORKING`, `PRINTER`, `STORAGE`, `POWER`, …).
    pub r#type: Option<String>,
    /// Model name, when the entry is specific enough to name one.
    pub model: Option<String>,
    /// Optional MIB-support module hint (the file's fifth field).
    pub module: Option<String>,
}

/// A parsed `sysobject.ids` database.
#[derive(Debug, Clone, Default)]
pub struct SysObjectIds {
    entries: HashMap<String, SysObjectEntry>,
}

/// Enterprise-OID prefixes recognised before the manufacturer id, longest /
/// most-specific first so `strip_prefix` cannot mis-bind the leading-dot form.
const ENTERPRISE_PREFIXES: [&str; 4] = [
    "SNMPv2-SMI::enterprises.",
    "iso.3.6.1.4.1.",
    ".1.3.6.1.4.1.",
    "1.3.6.1.4.1.",
];

impl SysObjectIds {
    /// Parses the contents of a `sysobject.ids` file.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut entries = HashMap::new();
        for raw in text.lines() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            let Some(id) = fields.next().filter(|id| !id.is_empty()) else {
                continue;
            };
            entries.insert(
                id.to_owned(),
                SysObjectEntry {
                    manufacturer: non_empty(fields.next()),
                    r#type: non_empty(fields.next()),
                    model: non_empty(fields.next()),
                    module: non_empty(fields.next()),
                },
            );
        }
        Self { entries }
    }

    /// Looks up the classification for a device `sysObjectID`.
    ///
    /// Accepts the numeric dotted form (with or without a leading dot) and the
    /// textual `iso.3.6.1.4.1.…` / `SNMPv2-SMI::enterprises.…` forms. Returns
    /// `None` if the OID is not under the enterprises arc or no entry matches.
    #[must_use]
    pub fn lookup(&self, sysobjectid: &str) -> Option<&SysObjectEntry> {
        let relative = enterprise_relative(sysobjectid)?;
        let (manufacturer, device) = match relative.split_once('.') {
            Some((m, d)) => (m, Some(d)),
            None => (relative, None),
        };
        if manufacturer.is_empty() || !is_numeric_arc(manufacturer) {
            return None;
        }

        if let Some(device) = device.filter(|d| is_numeric_oid(d)) {
            // Full match, then progressively shorter device-id prefixes.
            if let Some(entry) = self.entries.get(&format!("{manufacturer}.{device}")) {
                return Some(entry);
            }
            let mut partial = device;
            while let Some((head, _)) = partial.rsplit_once('.') {
                if let Some(entry) = self.entries.get(&format!("{manufacturer}.{head}")) {
                    return Some(entry);
                }
                partial = head;
            }
        }

        // Fallback: manufacturer id alone.
        self.entries.get(manufacturer)
    }

    /// Returns the number of entries loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no entries were loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Strips a recognised enterprise prefix, returning the relative OID after it.
fn enterprise_relative(oid: &str) -> Option<&str> {
    ENTERPRISE_PREFIXES
        .iter()
        .find_map(|prefix| oid.strip_prefix(prefix))
}

/// `true` if `s` is a non-empty run of ASCII digits.
fn is_numeric_arc(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `true` if `s` is a dotted sequence of numeric arcs (no empty arcs).
fn is_numeric_oid(s: &str) -> bool {
    !s.is_empty() && s.split('.').all(is_numeric_arc)
}

/// Maps a present-but-empty field to `None`, otherwise owns the string.
fn non_empty(field: Option<&str>) -> Option<String> {
    field.filter(|s| !s.is_empty()).map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::SysObjectIds;

    // Tab-separated; mirrors the upstream layout, including a comment, a blank
    // line, a manufacturer-only entry, and entries of varying specificity.
    const FIXTURE: &str = "# list of SysObject ID's\n\
        \n\
        1\tProteon\tNETWORKING\n\
        9\tCisco\tNETWORKING\n\
        9.1\tCisco\tNETWORKING\tGeneric Router\n\
        9.1.3\tCisco\tNETWORKING\tRouter xGS\n";

    fn db() -> SysObjectIds {
        SysObjectIds::parse(FIXTURE)
    }

    #[test]
    fn parses_entries_skipping_comments_and_blanks() {
        let db = db();
        assert_eq!(db.len(), 4);
        let entry = db.lookup("1.3.6.1.4.1.9.1.3").unwrap();
        assert_eq!(entry.manufacturer.as_deref(), Some("Cisco"));
        assert_eq!(entry.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(entry.model.as_deref(), Some("Router xGS"));
        assert_eq!(entry.module, None);
    }

    #[test]
    fn full_match_wins() {
        let db = db();
        assert_eq!(
            db.lookup("1.3.6.1.4.1.9.1.3").unwrap().model.as_deref(),
            Some("Router xGS")
        );
    }

    #[test]
    fn falls_back_to_shorter_device_prefixes() {
        let db = db();
        // 9.1.3.99 -> strip .99 -> 9.1.3 (Router xGS).
        assert_eq!(
            db.lookup("1.3.6.1.4.1.9.1.3.99").unwrap().model.as_deref(),
            Some("Router xGS")
        );
        // 9.1.7 -> no full, strip .7 -> 9.1 (Generic Router).
        assert_eq!(
            db.lookup("1.3.6.1.4.1.9.1.7").unwrap().model.as_deref(),
            Some("Generic Router")
        );
    }

    #[test]
    fn falls_back_to_manufacturer_only() {
        let db = db();
        let entry = db.lookup("1.3.6.1.4.1.9.50.50").unwrap();
        assert_eq!(entry.manufacturer.as_deref(), Some("Cisco"));
        assert_eq!(entry.model, None);
    }

    #[test]
    fn manufacturer_id_with_no_device_matches_manufacturer_entry() {
        let db = db();
        assert_eq!(
            db.lookup("1.3.6.1.4.1.1").unwrap().manufacturer.as_deref(),
            Some("Proteon")
        );
    }

    #[test]
    fn accepts_leading_dot_and_textual_prefixes() {
        let db = db();
        for oid in [
            ".1.3.6.1.4.1.9.1.3",
            "iso.3.6.1.4.1.9.1.3",
            "SNMPv2-SMI::enterprises.9.1.3",
        ] {
            assert_eq!(
                db.lookup(oid).unwrap().model.as_deref(),
                Some("Router xGS"),
                "prefix form {oid} should resolve"
            );
        }
    }

    #[test]
    fn non_enterprise_or_unknown_oids_return_none() {
        let db = db();
        // Not under the enterprises arc.
        assert!(db.lookup("1.3.6.1.2.1.1.1.0").is_none());
        // Unknown manufacturer id.
        assert!(db.lookup("1.3.6.1.4.1.99999.1").is_none());
        // Garbage.
        assert!(db.lookup("not-an-oid").is_none());
    }

    #[test]
    fn empty_trailing_fields_become_none() {
        let db = SysObjectIds::parse("9\tCisco\tNETWORKING\t\t\n");
        let entry = db.lookup("1.3.6.1.4.1.9").unwrap();
        assert_eq!(entry.model, None);
        assert_eq!(entry.module, None);
    }
}
