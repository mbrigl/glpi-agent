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

/// Detects installed antivirus products (Windows) from the Security Center.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Antivirus> {
    crate::sys::powershell(
        "Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct | \
         Select-Object displayName | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_antivirus(&json))
    .unwrap_or_default()
}

/// Detects installed antivirus products (other platforms: not yet implemented).
///
/// macOS has no common security-center API; endpoint products would need the
/// per-vendor marker approach, left for a follow-up.
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Antivirus> {
    Vec::new()
}

/// Parses a `SecurityCenter2 AntiVirusProduct` `ConvertTo-Json` result into the
/// registered products (presence implies enabled, as on Linux).
#[must_use]
pub fn parse_win_antivirus(json: &str) -> Vec<Antivirus> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            Some(Antivirus {
                name: crate::jsonutil::str_field(item, "displayName")?,
                enabled: true,
            })
        })
        .collect()
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

    #[test]
    fn parses_windows_securitycenter_json() {
        use super::parse_win_antivirus;
        let json = r#"[{"displayName":"Microsoft Defender"},{"displayName":"Acme AV"}]"#;
        let products = parse_win_antivirus(json);
        assert_eq!(products.len(), 2);
        assert_eq!(products[0].name, "Microsoft Defender");
        assert!(products.iter().all(|a| a.enabled));
        assert!(parse_win_antivirus("bad").is_empty());
    }
}
