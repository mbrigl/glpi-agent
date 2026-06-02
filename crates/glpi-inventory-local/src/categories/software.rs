// SPDX-License-Identifier: GPL-2.0-only

//! Software inventory category (Linux packages).
//!
//! Reads installed packages from the system package manager in a fixed
//! tab-separated `name<TAB>version<TAB>arch` form, so a single parser handles
//! both `dpkg-query -W` (Debian/Ubuntu) and `rpm -qa --qf …` (RHEL/SUSE). The
//! parser is pure and unit-tested; the live collector tries dpkg then rpm.

use serde::Serialize;

/// An installed software package.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Software {
    /// Package name.
    pub name: String,
    /// Package version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Package architecture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

/// Parses `name<TAB>version<TAB>arch` lines (as emitted by the configured
/// `dpkg-query` / `rpm` query) into packages. Lines without a name are skipped.
#[must_use]
pub fn parse_packages(text: &str) -> Vec<Software> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let name = fields.next()?.trim();
            if name.is_empty() {
                return None;
            }
            Some(Software {
                name: name.to_owned(),
                version: non_empty(fields.next()),
                arch: non_empty(fields.next()),
            })
        })
        .collect()
}

/// Collects installed packages, trying dpkg then rpm (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Software> {
    if let Some(out) = run(
        "dpkg-query",
        &["-W", "-f=${Package}\t${Version}\t${Architecture}\n"],
    ) {
        return parse_packages(&out);
    }
    if let Some(out) = run(
        "rpm",
        &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\n"],
    ) {
        return parse_packages(&out);
    }
    Vec::new()
}

/// Collects installed applications (macOS) from `system_profiler`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Software> {
    crate::sys::output("system_profiler", &["-json", "SPApplicationsDataType"])
        .map(|json| parse_macos_software(&json))
        .unwrap_or_default()
}

/// Collects installed programs (Windows) from the registry uninstall keys
/// (per-machine 64- and 32-bit, plus per-user).
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Software> {
    let script = "$paths=@(\
        'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',\
        'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',\
        'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*'); \
        Get-ItemProperty $paths -ErrorAction SilentlyContinue | \
        Where-Object {$_.DisplayName} | \
        Select-Object DisplayName,DisplayVersion,Publisher | ConvertTo-Json -Compress";
    crate::sys::powershell(script)
        .map(|json| parse_win_software(&json))
        .unwrap_or_default()
}

/// Collects installed packages (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Software> {
    Vec::new()
}

/// Parses the Windows registry uninstall entries (`ConvertTo-Json`) into the
/// installed software; entries without a `DisplayName` are skipped.
#[must_use]
pub fn parse_win_software(json: &str) -> Vec<Software> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            Some(Software {
                name: crate::jsonutil::str_field(item, "DisplayName")?,
                version: crate::jsonutil::str_field(item, "DisplayVersion"),
                arch: None,
            })
        })
        .collect()
}

/// Parses `system_profiler -json SPApplicationsDataType` (macOS) into the
/// installed applications.
#[must_use]
pub fn parse_macos_software(json: &str) -> Vec<Software> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    value
        .get("SPApplicationsDataType")
        .and_then(serde_json::Value::as_array)
        .map(|apps| {
            apps.iter()
                .filter_map(|item| {
                    Some(Software {
                        name: crate::jsonutil::str_field(item, "_name")?,
                        version: crate::jsonutil::str_field(item, "version"),
                        arch: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Runs `command args`, returning its stdout on success.
#[cfg(target_os = "linux")]
fn run(command: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(command)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Trims a field and maps an empty one to `None`.
fn non_empty(field: Option<&str>) -> Option<String> {
    field
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::parse_packages;

    #[test]
    fn parses_dpkg_query_output() {
        let text = "bash\t5.2.15-2+b7\tamd64\ncoreutils\t9.1-1\tamd64\n";
        let pkgs = parse_packages(text);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "bash");
        assert_eq!(pkgs[0].version.as_deref(), Some("5.2.15-2+b7"));
        assert_eq!(pkgs[0].arch.as_deref(), Some("amd64"));
        assert_eq!(pkgs[1].name, "coreutils");
    }

    #[test]
    fn tolerates_missing_arch_and_blank_lines() {
        let text = "vim\t9.0\n\nsomepkg\n";
        let pkgs = parse_packages(text);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "vim");
        assert_eq!(pkgs[0].arch, None);
        // Name-only line: still a package, no version/arch.
        assert_eq!(pkgs[1].name, "somepkg");
        assert_eq!(pkgs[1].version, None);
    }

    #[test]
    fn empty_input_yields_no_packages() {
        assert!(parse_packages("").is_empty());
    }

    #[test]
    fn parses_windows_uninstall_json() {
        use super::parse_win_software;
        // The entry without a DisplayName (an orphan version) is skipped.
        let json = r#"[{"DisplayName":"7-Zip 23.01","DisplayVersion":"23.01","Publisher":"Igor"},
            {"DisplayVersion":"1.0"},
            {"DisplayName":"Mozilla Firefox","DisplayVersion":"123.0"}]"#;
        let apps = parse_win_software(json);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "7-Zip 23.01");
        assert_eq!(apps[0].version.as_deref(), Some("23.01"));
        assert_eq!(apps[1].name, "Mozilla Firefox");
        assert!(parse_win_software("bad").is_empty());
    }

    #[test]
    fn parses_macos_applications_json() {
        use super::parse_macos_software;
        let json = r#"{"SPApplicationsDataType":[{"_name":"Safari","version":"17.0"},
            {"_name":"Xcode","version":"15.3"},{"version":"1.0"}]}"#;
        let apps = parse_macos_software(json);
        // The entry without a name is skipped.
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "Safari");
        assert_eq!(apps[1].version.as_deref(), Some("15.3"));
    }
}
