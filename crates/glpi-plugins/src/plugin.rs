// SPDX-License-Identifier: GPL-2.0-only

//! The HTTP-server plugin trait and small config-parsing helpers.
//!
//! Plugins extend the embedded HTTP server (`glpi-http`). Each carries a config
//! (loaded from its `*.cfg` file), advertises whether it is enabled and on
//! which extra port it listens. The request handling itself is wired by the
//! server; this crate models the plugins' configuration and decision logic.

use std::collections::BTreeMap;

/// An HTTP-server plugin's identity and listener configuration.
pub trait Plugin {
    /// Stable plugin name (for logs).
    fn name(&self) -> &'static str;

    /// The plugin's configuration file name (`*.cfg`).
    fn config_file(&self) -> &'static str;

    /// Whether the plugin is disabled.
    fn is_disabled(&self) -> bool;

    /// Extra listener port (`0` = share the agent's main HTTP port).
    fn port(&self) -> u16;
}

/// Parses a boolean config value: `1`/`yes`/`true` are true; `0`/`no`/`false`
/// and anything else are false (matching the upstream `!~ /^0|no$/i` checks).
#[must_use]
pub fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "yes" | "true"
    )
}

/// Looks up `key` in a config map and parses it as a bool, or returns
/// `default` when absent.
#[must_use]
pub fn bool_or(config: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    config.get(key).map_or(default, |v| parse_bool(v))
}

/// Looks up `key` and parses it as a number, or returns `default`.
#[must_use]
pub fn parse_or<T: std::str::FromStr>(
    config: &BTreeMap<String, String>,
    key: &str,
    default: T,
) -> T {
    config
        .get(key)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// Looks up `key` and returns it as a non-empty owned string, else `None`.
#[must_use]
pub fn string_opt(config: &BTreeMap<String, String>, key: &str) -> Option<String> {
    config
        .get(key)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{bool_or, parse_bool, parse_or, string_opt};
    use std::collections::BTreeMap;

    #[test]
    fn parses_booleans_like_upstream() {
        assert!(parse_bool("yes") && parse_bool("1") && parse_bool("YES"));
        assert!(!parse_bool("no") && !parse_bool("0") && !parse_bool(""));
    }

    #[test]
    fn config_helpers_fall_back_to_defaults() {
        let mut cfg = BTreeMap::new();
        cfg.insert("port".to_owned(), "8443".to_owned());
        cfg.insert("local_store".to_owned(), "  ".to_owned());
        assert_eq!(parse_or(&cfg, "port", 0u16), 8443);
        assert_eq!(parse_or(&cfg, "missing", 7u16), 7);
        assert!(bool_or(&cfg, "missing", true));
        // Blank value -> None.
        assert_eq!(string_opt(&cfg, "local_store"), None);
    }
}
