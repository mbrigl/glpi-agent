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

/// Collects installed packages (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Software> {
    Vec::new()
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
}
