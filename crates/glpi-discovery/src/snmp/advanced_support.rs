// SPDX-License-Identifier: GPL-2.0-only

//! `snmp-advanced-support.cfg` parser.
//!
//! This config customizes the *session test* the agent uses to decide whether
//! an address hosts a reachable SNMP device. By default only
//! `sysDescr` (`.1.3.6.1.2.1.1.1.0`) is checked, but some edge devices (e.g.
//! Snom IP phones) do not answer it, so the file lets an operator extend the
//! list:
//!
//! ```text
//! # comma-separated OIDs; the device is reachable if ANY of them answers
//! oids = .1.3.6.1.2.1.1.1.0,.1.3.6.1.2.1.7526.2.4
//! include "snmp-advanced-support.local"
//! ```
//!
//! The file is `key = value` with `#` comments and an `include "path"`
//! directive. Only the `oids` key is defined; unknown keys are ignored. This
//! parser is pure over the file text — resolving `include` paths against the
//! filesystem is the loader's job; [`AdvancedSupport::includes`] records them
//! in order.

use glpi_core::error::{AgentError, Result};

use crate::snmp::query::SYS_DESCR;

/// Parsed contents of an `snmp-advanced-support.cfg` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdvancedSupport {
    /// Session-test OIDs (arc form), or empty if the file set none.
    pub oids: Vec<Vec<u64>>,
    /// `include` directives, in the order they appear.
    pub includes: Vec<String>,
}

impl AdvancedSupport {
    /// Parses the file text.
    ///
    /// The last `oids =` assignment wins (key/value override semantics). A line
    /// is one of: a comment (`#…`), blank, an `include "path"` directive, or a
    /// `key = value` pair; unknown keys are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if an `oids` value contains a malformed
    /// OID, or an `include` directive is not a quoted path.
    pub fn parse(text: &str) -> Result<Self> {
        let mut oids: Option<Vec<Vec<u64>>> = None;
        let mut includes = Vec::new();

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("include") {
                includes.push(parse_include(rest)?);
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() == "oids" {
                oids = Some(parse_oid_list(value.trim())?);
            }
        }

        Ok(Self {
            oids: oids.unwrap_or_default(),
            includes,
        })
    }

    /// Returns the OIDs to use for the session test: the configured list if
    /// non-empty, otherwise the default (`sysDescr` alone).
    #[must_use]
    pub fn session_test_oids(&self) -> Vec<Vec<u64>> {
        if self.oids.is_empty() {
            vec![SYS_DESCR.to_vec()]
        } else {
            self.oids.clone()
        }
    }
}

/// Parses a comma-separated OID list, ignoring empty entries.
fn parse_oid_list(value: &str) -> Result<Vec<Vec<u64>>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(parse_dotted_oid)
        .collect()
}

/// Parses a dotted OID with an optional leading dot into numeric arcs.
fn parse_dotted_oid(oid: &str) -> Result<Vec<u64>> {
    let oid = oid.strip_prefix('.').unwrap_or(oid);
    oid.split('.')
        .map(|arc| {
            arc.parse::<u64>().map_err(|_| {
                AgentError::Config(format!("invalid OID in advanced-support: {oid:?}"))
            })
        })
        .collect()
}

/// Extracts the quoted path of an `include "path"` directive.
fn parse_include(rest: &str) -> Result<String> {
    let trimmed = rest.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AgentError::Config(format!("malformed include directive: {trimmed:?}")))
}

#[cfg(test)]
mod tests {
    use super::AdvancedSupport;
    use crate::snmp::query::SYS_DESCR;

    // The shipped file, with everything commented out.
    const DEFAULT_FILE: &str = "\
# oids is a comma-separated list of oids used during session testing.
#oids = .1.3.6.1.2.1.1.1.0
#oids = .1.3.6.1.2.1.1.1.0,.1.3.6.1.2.1.7526.2.4
include \"snmp-advanced-support.local\"
";

    #[test]
    fn default_file_yields_no_oids_but_records_include() {
        let cfg = AdvancedSupport::parse(DEFAULT_FILE).unwrap();
        assert!(cfg.oids.is_empty());
        assert_eq!(cfg.includes, vec!["snmp-advanced-support.local".to_owned()]);
        // Falls back to sysDescr for the session test.
        assert_eq!(cfg.session_test_oids(), vec![SYS_DESCR.to_vec()]);
    }

    #[test]
    fn parses_snom_example_with_leading_dots() {
        let cfg =
            AdvancedSupport::parse("oids = .1.3.6.1.2.1.1.1.0,.1.3.6.1.2.1.7526.2.4\n").unwrap();
        assert_eq!(
            cfg.oids,
            vec![
                vec![1, 3, 6, 1, 2, 1, 1, 1, 0],
                vec![1, 3, 6, 1, 2, 1, 7526, 2, 4],
            ]
        );
        assert_eq!(cfg.session_test_oids(), cfg.oids);
    }

    #[test]
    fn last_oids_assignment_wins() {
        let cfg =
            AdvancedSupport::parse("oids = .1.3.6.1.2.1.1.1.0\noids = .1.3.6.1.4.1.9\n").unwrap();
        assert_eq!(cfg.oids, vec![vec![1, 3, 6, 1, 4, 1, 9]]);
    }

    #[test]
    fn tolerates_oids_without_leading_dot_and_whitespace() {
        let cfg = AdvancedSupport::parse("oids = 1.3.6.1 , .1.3.6.2\n").unwrap();
        assert_eq!(cfg.oids, vec![vec![1, 3, 6, 1], vec![1, 3, 6, 2]]);
    }

    #[test]
    fn rejects_malformed_oid() {
        assert!(AdvancedSupport::parse("oids = .1.3.x.4\n").is_err());
    }

    #[test]
    fn rejects_unquoted_include() {
        assert!(AdvancedSupport::parse("include foo.local\n").is_err());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let cfg = AdvancedSupport::parse("verbose = yes\noids = .1.3\n").unwrap();
        assert_eq!(cfg.oids, vec![vec![1, 3]]);
    }
}
