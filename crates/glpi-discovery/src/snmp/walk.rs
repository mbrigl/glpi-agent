// SPDX-License-Identifier: GPL-2.0-only

//! In-memory SNMP source from a captured `snmpwalk`.
//!
//! [`WalkSession`] parses the standard numeric `snmpwalk` text format
//! (`snmpwalk -On`) into an OID→[`SnmpValue`] map and serves it through the
//! [`SnmpQuery`] trait. This drives offline tests and fixture replay: the
//! upstream agent's `resources/walks/*.walk` captures (and any `snmpwalk -On`
//! output) load directly and exercise the full interpretation chain
//! (`identify`, `discover_snmp`, the MIB modules) without a live device.
//!
//! Recognized line shape: `OID = TYPE: value`, with continuation lines (no
//! `=`) appended to the previous value — net-snmp wraps long `Hex-STRING`s this
//! way. Handled types: `STRING`, `Hex-STRING`, `OID`, `Timeticks`, `INTEGER`,
//! `Counter32`, `Counter64`, `Gauge32`, `IpAddress`, plus the bare empty
//! string `""`. Any other type is kept leniently as an `OctetString` of its
//! raw text so a fixture never fails to load.

use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};

use super::query::SnmpQuery;
use super::value::SnmpValue;

/// An SNMP source backed by a parsed `snmpwalk` capture.
#[derive(Debug, Clone, Default)]
pub struct WalkSession {
    entries: BTreeMap<Vec<u64>, SnmpValue>,
}

impl WalkSession {
    /// Parses numeric `snmpwalk` output into a session.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Parse`] if a record's OID is not a valid
    /// dotted-decimal identifier.
    pub fn parse(text: &str) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for record in coalesce_records(text) {
            if let Some((oid, value)) = parse_record(&record)? {
                entries.insert(oid, value);
            }
        }
        Ok(Self { entries })
    }

    /// Number of varbinds loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if the walk loaded no varbinds.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[async_trait]
impl SnmpQuery for WalkSession {
    async fn get(&mut self, oid: &[u64]) -> Result<Option<SnmpValue>> {
        Ok(self.entries.get(oid).cloned())
    }

    async fn get_next(&mut self, oid: &[u64]) -> Result<Option<(Vec<u64>, SnmpValue)>> {
        Ok(self
            .entries
            .range((Excluded(oid.to_vec()), Unbounded))
            .next()
            .map(|(k, v)| (k.clone(), v.clone())))
    }

    async fn walk(&mut self, root: &[u64]) -> Result<Vec<(Vec<u64>, SnmpValue)>> {
        Ok(self
            .entries
            .iter()
            .filter(|(k, _)| k.starts_with(root))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

/// Joins continuation lines (those without an `=` after an OID) onto the
/// preceding record, returning one string per varbind.
fn coalesce_records(text: &str) -> Vec<String> {
    let mut records: Vec<String> = Vec::new();
    for line in text.lines() {
        if is_record_start(line) {
            records.push(line.trim().to_owned());
        } else if let Some(last) = records.last_mut() {
            last.push(' ');
            last.push_str(line.trim());
        }
        // A continuation with no preceding record is leading junk: ignore.
    }
    records
}

/// `true` if `line` begins a new varbind (`<oid> = …`).
fn is_record_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    matches!(trimmed.bytes().next(), Some(b'.' | b'0'..=b'9')) && trimmed.contains(" = ")
}

/// Parses one coalesced `OID = TYPE: value` record.
fn parse_record(record: &str) -> Result<Option<(Vec<u64>, SnmpValue)>> {
    let Some((oid, rest)) = record.split_once(" = ") else {
        return Ok(None);
    };
    let oid = parse_oid(oid.trim())?;
    Ok(Some((oid, parse_value(rest.trim()))))
}

/// Parses a dotted OID (optional leading dot) into numeric arcs.
fn parse_oid(oid: &str) -> Result<Vec<u64>> {
    let oid = oid.strip_prefix('.').unwrap_or(oid);
    oid.split('.')
        .map(|arc| {
            arc.parse::<u64>()
                .map_err(|_| AgentError::Parse(format!("invalid OID in walk: {oid:?}")))
        })
        .collect()
}

/// Parses the `TYPE: value` portion into an [`SnmpValue`], leniently.
fn parse_value(rest: &str) -> SnmpValue {
    if rest == "\"\"" || rest.is_empty() {
        return SnmpValue::OctetString(Vec::new());
    }
    let Some((kind, value)) = rest.split_once(": ") else {
        // No explicit type (some captures print a bare string).
        return SnmpValue::OctetString(unquote(rest).as_bytes().to_vec());
    };
    let value = value.trim();
    match kind {
        "STRING" => SnmpValue::OctetString(unquote(value).into_bytes()),
        "Hex-STRING" => SnmpValue::OctetString(parse_hex(value)),
        "OID" => SnmpValue::Oid(value.strip_prefix('.').unwrap_or(value).to_owned()),
        "Timeticks" => SnmpValue::Timeticks(parenthesized_number(value).unwrap_or(0)),
        "INTEGER" => SnmpValue::Integer(integer_value(value).unwrap_or(0)),
        "Counter32" => SnmpValue::Counter32(value.parse().unwrap_or(0)),
        "Counter64" => SnmpValue::Counter64(value.parse().unwrap_or(0)),
        "Gauge32" => SnmpValue::Unsigned32(value.parse().unwrap_or(0)),
        "IpAddress" => parse_ipv4(value)
            .map(SnmpValue::IpAddress)
            .unwrap_or_else(|| SnmpValue::OctetString(value.as_bytes().to_vec())),
        // Unknown type: keep the raw text so the fixture still loads.
        _ => SnmpValue::OctetString(value.as_bytes().to_vec()),
    }
}

/// Strips one layer of surrounding double quotes, if present.
fn unquote(s: &str) -> String {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
        .to_owned()
}

/// Parses space-separated hex byte pairs (`00 1A 2B`) into bytes.
fn parse_hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .filter_map(|byte| u8::from_str_radix(byte, 16).ok())
        .collect()
}

/// Extracts the unsigned number inside parentheses, e.g. `(12345)` → 12345.
fn parenthesized_number(s: &str) -> Option<u32> {
    let start = s.find('(')?;
    let end = s[start..].find(')')? + start;
    s[start + 1..end].parse().ok()
}

/// Parses an `INTEGER` value: either a bare number or an enum `name(N)`.
fn integer_value(s: &str) -> Option<i64> {
    if s.contains('(') {
        parenthesized_number(s).map(i64::from)
    } else {
        s.parse().ok()
    }
}

/// Parses a dotted IPv4 literal into four octets.
fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let mut octets = [0u8; 4];
    let mut parts = s.split('.');
    for octet in &mut octets {
        *octet = parts.next()?.trim().parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(octets)
}

#[cfg(test)]
mod tests {
    use super::WalkSession;
    use crate::snmp::query::{identify, SnmpQuery};
    use crate::snmp::sysobject::SysObjectIds;
    use crate::snmp::value::SnmpValue;
    use crate::tasks::net_discovery::discover_snmp;

    const CISCO_WALK: &str = r#".1.3.6.1.2.1.1.1.0 = STRING: "Cisco IOS Software, C2960"
.1.3.6.1.2.1.1.2.0 = OID: .1.3.6.1.4.1.9.1.3
.1.3.6.1.2.1.1.3.0 = Timeticks: (123456789) 14 days, 6:56:07.89
.1.3.6.1.2.1.1.4.0 = STRING: "netops@example.com"
.1.3.6.1.2.1.1.5.0 = STRING: "core-sw-1"
.1.3.6.1.2.1.1.6.0 = STRING: "Rack 4"
.1.3.6.1.2.1.2.2.1.6.1 = Hex-STRING: 00 1A 2B 3C 4D 5E
.1.3.6.1.2.1.2.2.1.5.1 = Gauge32: 1000000000
.1.3.6.1.2.1.4.20.1.1.10.0.0.1 = IpAddress: 10.0.0.1
"#;

    #[tokio::test]
    async fn parses_common_types() {
        let mut walk = WalkSession::parse(CISCO_WALK).unwrap();
        assert_eq!(walk.len(), 9);

        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 1, 1, 0]).await.unwrap(),
            Some(SnmpValue::OctetString(
                b"Cisco IOS Software, C2960".to_vec()
            ))
        );
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 1, 2, 0]).await.unwrap(),
            Some(SnmpValue::Oid("1.3.6.1.4.1.9.1.3".to_owned()))
        );
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 1, 3, 0]).await.unwrap(),
            Some(SnmpValue::Timeticks(123_456_789))
        );
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 2, 2, 1, 6, 1]).await.unwrap(),
            Some(SnmpValue::OctetString(vec![
                0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e
            ]))
        );
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 2, 2, 1, 5, 1]).await.unwrap(),
            Some(SnmpValue::Unsigned32(1_000_000_000))
        );
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 4, 20, 1, 1, 10, 0, 0, 1])
                .await
                .unwrap(),
            Some(SnmpValue::IpAddress([10, 0, 0, 1]))
        );
    }

    #[tokio::test]
    async fn walk_returns_subtree_in_order() {
        let mut walk = WalkSession::parse(CISCO_WALK).unwrap();
        let system = walk.walk(&[1, 3, 6, 1, 2, 1, 1]).await.unwrap();
        // sysDescr, sysObjectID, sysUpTime, sysContact, sysName, sysLocation.
        assert_eq!(system.len(), 6);
        assert!(system.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[tokio::test]
    async fn integer_enum_form_takes_parenthesized_number() {
        let mut walk = WalkSession::parse(".1.3.6.1.2.1.2.2.1.8.1 = INTEGER: up(1)\n").unwrap();
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 2, 2, 1, 8, 1]).await.unwrap(),
            Some(SnmpValue::Integer(1))
        );
    }

    #[tokio::test]
    async fn continuation_lines_extend_hex_strings() {
        let walk =
            WalkSession::parse(".1.3.6.1.2.1.1.1.0 = Hex-STRING: 00 11 22 33\n44 55 66 77\n")
                .unwrap();
        let mut walk = walk;
        assert_eq!(
            walk.get(&[1, 3, 6, 1, 2, 1, 1, 1, 0]).await.unwrap(),
            Some(SnmpValue::OctetString(vec![
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77
            ]))
        );
    }

    #[tokio::test]
    async fn drives_the_full_identification_chain() {
        let mut walk = WalkSession::parse(CISCO_WALK).unwrap();
        let sysobjects = SysObjectIds::parse("9.1.3\tCisco\tNETWORKING\tCatalyst 2960\n");

        // identify reads the system group.
        let info = identify(&mut walk).await.unwrap().unwrap();
        assert_eq!(info.sys_name.as_deref(), Some("core-sw-1"));
        assert_eq!(info.sys_object_id.as_deref(), Some("1.3.6.1.4.1.9.1.3"));

        // discover_snmp classifies it via the sysobject database.
        let device = discover_snmp(&mut walk, &sysobjects)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(device.manufacturer.as_deref(), Some("Cisco"));
        assert_eq!(device.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.model.as_deref(), Some("Catalyst 2960"));
        assert_eq!(device.contact.as_deref(), Some("netops@example.com"));
        assert_eq!(device.location.as_deref(), Some("Rack 4"));
    }

    #[test]
    fn rejects_invalid_oid() {
        assert!(WalkSession::parse(".1.3.x.4 = STRING: nope\n").is_err());
    }
}
