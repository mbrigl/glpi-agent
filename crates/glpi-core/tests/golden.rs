// SPDX-License-Identifier: GPL-2.0-only

//! Golden-file tests for the GLPI native JSON protocol.
//!
//! This is the seed of the parity harness described in the migration plan: a
//! message is built from typed values, serialized, and compared against a
//! committed fixture that represents the expected wire output. Phase 2 onward
//! reuses the same `load_fixture` + normalized-`Value` comparison against
//! captures taken from the upstream Perl agent.
//!
//! Comparing parsed [`serde_json::Value`]s (rather than raw strings) normalizes
//! away key ordering and whitespace, so a fixture stays readable and stable.

use glpi_core::protocol::{ContactRequest, InventoryRequest};
use serde_json::{json, Value};
use std::path::Path;

/// Loads and parses a JSON fixture from `tests/fixtures/<name>`.
fn load_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("fixture {} is not valid JSON: {e}", path.display()))
}

#[test]
fn contact_request_matches_golden() {
    let mut request = ContactRequest::new("agent-golden-1");
    request.tag = Some("lab".to_owned());
    request.enabled_tasks = vec!["inventory".to_owned(), "netdiscovery".to_owned()];

    let produced = serde_json::to_value(&request).unwrap();
    assert_eq!(produced, load_fixture("contact_request.json"));
}

#[test]
fn inventory_request_matches_golden() {
    let request = InventoryRequest::new(
        "agent-golden-1",
        json!({ "hardware": { "name": "host-01" } }),
    );

    let produced = serde_json::to_value(&request).unwrap();
    assert_eq!(produced, load_fixture("inventory_request.json"));
}
