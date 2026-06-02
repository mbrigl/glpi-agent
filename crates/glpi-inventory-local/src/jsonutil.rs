// SPDX-License-Identifier: GPL-2.0-only

//! Small `serde_json::Value` accessors shared by the Windows/macOS collectors.
//!
//! The Windows (`Get-CimInstance … | ConvertTo-Json`) and macOS
//! (`system_profiler -json …`) collectors parse JSON; these helpers read fields
//! tolerantly (a single object or a one-element array, numbers that may arrive
//! as quoted strings). They are pure and compiled on every platform so the
//! parsers stay unit-testable on Linux.

use serde_json::Value;

/// Normalizes a `ConvertTo-Json` value (a bare object or an array) into a list
/// of items; `null` becomes empty.
#[must_use]
pub(crate) fn array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

/// Reads a non-empty, trimmed string field from a JSON object.
#[must_use]
pub(crate) fn str_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Reads an unsigned-integer field, accepting a numeric string too (some CIM
/// exports quote large values such as `Capacity`).
#[must_use]
pub(crate) fn u64_field(value: &Value, key: &str) -> Option<u64> {
    let field = value.get(key)?;
    field
        .as_u64()
        .or_else(|| field.as_str().and_then(|s| s.trim().parse().ok()))
}
