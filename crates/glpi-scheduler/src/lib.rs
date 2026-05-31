// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-scheduler` — Daemon scheduling, events and task forking.
//!
//! Part of the GLPI Agent Rust workspace (v2.0.0).
//! Placeholder crate; implementation lands in a later phase.

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
