// SPDX-License-Identifier: GPL-2.0-only

//! Antivirus inventory category (Linux, presence-based detection).
//!
//! Linux endpoint-security products don't expose a common API, so they're
//! detected by the presence of their well-known install markers (a control
//! binary or install directory). [`detect_present`] takes an existence
//! predicate so it is pure and unit-testable; the live collector checks the
//! filesystem.

use serde::Serialize;

/// A detected antivirus / endpoint-security product.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Antivirus {
    /// Product name.
    pub name: String,
    /// Whether the product appears installed/active.
    pub enabled: bool,
}

/// Known Linux endpoint-security products and a marker path that indicates an
/// installation.
const KNOWN: &[(&str, &str)] = &[
    ("CrowdStrike Falcon", "/opt/CrowdStrike/falconctl"),
    ("SentinelOne", "/opt/sentinelone/bin/sentinelctl"),
    ("ESET", "/opt/eset/efs/lib/libesets_pac.so"),
    ("Sophos", "/opt/sophos-spl/bin/savdid"),
    ("Cortex XDR", "/opt/traps/bin/cytool"),
    ("Dr.Web", "/opt/drweb.com/bin/drweb-ctl"),
    ("Kaspersky", "/opt/kaspersky/kesl/bin/kesl-control"),
    ("Trellix", "/opt/McAfee/ens/tp/bin/mfetpd"),
];

/// Returns the products whose marker path satisfies `exists`.
#[must_use]
pub fn detect_present<F>(exists: F) -> Vec<Antivirus>
where
    F: Fn(&str) -> bool,
{
    KNOWN
        .iter()
        .filter(|(_, marker)| exists(marker))
        .map(|(name, _)| Antivirus {
            name: (*name).to_owned(),
            enabled: true,
        })
        .collect()
}

/// Detects installed antivirus products from the filesystem (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Antivirus> {
    detect_present(|path| std::path::Path::new(path).exists())
}

/// Detects installed antivirus products (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Antivirus> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::detect_present;

    #[test]
    fn detects_products_by_marker() {
        // Pretend only the CrowdStrike and ESET markers exist.
        let present =
            detect_present(|p| p == "/opt/CrowdStrike/falconctl" || p.starts_with("/opt/eset/"));
        let names: Vec<_> = present.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"CrowdStrike Falcon"));
        assert!(names.contains(&"ESET"));
        assert!(!names.contains(&"SentinelOne"));
        assert!(present.iter().all(|a| a.enabled));
    }

    #[test]
    fn nothing_installed_yields_empty() {
        assert!(detect_present(|_| false).is_empty());
    }
}
