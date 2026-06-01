// SPDX-License-Identifier: GPL-2.0-only

//! Environment-variable inventory category.
//!
//! Reports the agent process's environment as `(key, value)` pairs for the GLPI
//! `envs` section, sorted by key for deterministic output. Unlike the other
//! categories this needs no command — [`std::env::vars`] is cross-platform — so
//! [`collect`] works on every platform.

use serde::Serialize;

/// An environment variable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EnvVar {
    /// Variable name.
    pub key: String,
    /// Variable value.
    pub val: String,
}

/// Builds the sorted environment-variable list from `(key, value)` pairs.
#[must_use]
pub fn from_vars<I>(vars: I) -> Vec<EnvVar>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut envs: Vec<EnvVar> = vars
        .into_iter()
        .map(|(key, val)| EnvVar { key, val })
        .collect();
    envs.sort_by(|a, b| a.key.cmp(&b.key));
    envs
}

/// Collects the live process environment (all platforms).
#[must_use]
pub fn collect() -> Vec<EnvVar> {
    from_vars(std::env::vars())
}

#[cfg(test)]
mod tests {
    use super::from_vars;

    #[test]
    fn builds_sorted_pairs() {
        let envs = from_vars([
            ("PATH".to_owned(), "/usr/bin".to_owned()),
            ("HOME".to_owned(), "/root".to_owned()),
        ]);
        assert_eq!(envs.len(), 2);
        // Sorted by key.
        assert_eq!(envs[0].key, "HOME");
        assert_eq!(envs[0].val, "/root");
        assert_eq!(envs[1].key, "PATH");
    }

    #[test]
    fn empty_input_yields_no_vars() {
        assert!(from_vars([]).is_empty());
    }
}
