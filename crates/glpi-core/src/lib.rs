// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-core` — shared types, protocol, configuration, auth and logging
//! for the GLPI Agent Rust workspace (v2.0.0).
//!
//! This crate is the foundation every task crate builds on. Only a minimal
//! skeleton is present so far; the type, protocol, config, auth and logging
//! modules are filled in during Phase 1.

/// Returns this crate's package name (placeholder smoke-test symbol).
#[must_use]
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
