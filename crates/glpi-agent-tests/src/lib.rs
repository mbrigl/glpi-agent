// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-agent-tests` — cross-crate integration and parity tests (Phase 10).
//!
//! This crate carries no production code; it exists so the Phase 10
//! stabilization tests can exercise several crates together that no single
//! crate can host on its own — a task crate feeding the transport, the
//! transport talking to a mock GLPI server, and serialized output checked for
//! GLPI schema parity. The tests live under [`tests/`](../tests); this library
//! provides the shared helpers they use.

use serde_json::Value;

/// Recursively sorts JSON object keys so two values compare regardless of key
/// order — the normalization the golden/parity diffs rely on.
#[must_use]
pub fn normalize(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            // `serde_json::Map` with the default feature preserves insertion
            // order; collecting through a BTreeMap canonicalizes it.
            let sorted: std::collections::BTreeMap<String, Value> =
                map.into_iter().map(|(k, v)| (k, normalize(v))).collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use serde_json::json;

    #[test]
    fn normalize_is_key_order_independent() {
        let a = normalize(json!({ "b": 1, "a": { "y": 2, "x": 3 } }));
        let b = normalize(json!({ "a": { "x": 3, "y": 2 }, "b": 1 }));
        assert_eq!(a, b);
    }
}
