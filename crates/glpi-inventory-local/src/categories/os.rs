// SPDX-License-Identifier: GPL-2.0-only

//! Operating-system inventory category.
//!
//! On Linux the OS identity comes from `/etc/os-release` (the distro name,
//! version and pretty name) combined with the running kernel (`uname`-style
//! data from `/proc/sys/kernel`) and the build architecture. The
//! [`parse_os_release`] parser is pure and unit-tested; the live collector is a
//! thin Linux wrapper around it.

use std::collections::HashMap;

use serde::Serialize;

/// The `operatingsystem` inventory payload.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct OperatingSystem {
    /// Distribution name (`NAME` from `os-release`, e.g. "Ubuntu").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Version id (`VERSION_ID`, e.g. "22.04").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Full descriptive name (`PRETTY_NAME`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    /// Kernel name (e.g. "Linux").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_name: Option<String>,
    /// Kernel release (e.g. "6.1.0-13-amd64").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    /// CPU architecture (e.g. "x86_64").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// Fully-qualified domain name, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqdn: Option<String>,
    /// System timezone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<Timezone>,
}

/// The system timezone (`operatingsystem.timezone`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Timezone {
    /// IANA zone name (e.g. "Europe/Berlin").
    pub name: String,
    /// UTC offset (e.g. "+0100"), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
}

/// Extracts the IANA zone name from an `/etc/localtime` symlink target such as
/// `/usr/share/zoneinfo/Europe/Berlin` or `../usr/share/zoneinfo/UTC`.
#[must_use]
pub fn parse_timezone_name(localtime_target: &str) -> Option<String> {
    localtime_target
        .split_once("zoneinfo/")
        .map(|(_, zone)| zone.trim().to_owned())
        .filter(|zone| !zone.is_empty())
}

/// Collects the live operating-system identity (Linux).
///
/// Combines `/etc/os-release` with the running kernel (`/proc/sys/kernel`) and
/// the build architecture.
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> OperatingSystem {
    let mut os = std::fs::read_to_string("/etc/os-release")
        .map(|text| parse_os_release(&text))
        .unwrap_or_default();
    os.kernel_name = read_trimmed("/proc/sys/kernel/ostype");
    os.kernel_version = read_trimmed("/proc/sys/kernel/osrelease");
    os.arch = Some(std::env::consts::ARCH.to_owned());
    os.timezone = collect_timezone().map(|name| Timezone { name, offset: None });
    os
}

/// Determines the IANA timezone name from `/etc/timezone` or the
/// `/etc/localtime` symlink.
#[cfg(target_os = "linux")]
fn collect_timezone() -> Option<String> {
    read_trimmed("/etc/timezone").or_else(|| {
        std::fs::read_link("/etc/localtime")
            .ok()
            .and_then(|target| parse_timezone_name(&target.to_string_lossy()))
    })
}

/// Collects the live operating-system identity (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> OperatingSystem {
    OperatingSystem::default()
}

/// Reads a file and returns its trimmed contents, if any.
#[cfg(target_os = "linux")]
fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Parses `/etc/os-release` into the OS identity fields it provides.
///
/// Kernel and architecture are not in `os-release`; the live collector fills
/// those separately. Unknown/missing keys leave their fields `None`.
#[must_use]
pub fn parse_os_release(text: &str) -> OperatingSystem {
    let map = os_release_map(text);
    OperatingSystem {
        name: map.get("NAME").cloned(),
        version: map.get("VERSION_ID").cloned(),
        full_name: map.get("PRETTY_NAME").cloned(),
        ..OperatingSystem::default()
    }
}

/// Parses the `KEY=VALUE` lines of an `os-release` file, stripping surrounding
/// quotes and skipping comments and blank lines.
fn os_release_map(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_owned(), unquote(value.trim()).to_owned());
        }
    }
    map
}

/// Strips one layer of surrounding single or double quotes.
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0]
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::parse_os_release;

    const UBUNTU: &str = r#"NAME="Ubuntu"
VERSION="22.04.3 LTS (Jammy Jellyfish)"
ID=ubuntu
ID_LIKE=debian
PRETTY_NAME="Ubuntu 22.04.3 LTS"
VERSION_ID="22.04"
VERSION_CODENAME=jammy
"#;

    const DEBIAN: &str = "\
# a comment
PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"
NAME=\"Debian GNU/Linux\"
VERSION_ID=\"12\"
VERSION=\"12 (bookworm)\"
ID=debian
";

    #[test]
    fn parses_ubuntu_identity() {
        let os = parse_os_release(UBUNTU);
        assert_eq!(os.name.as_deref(), Some("Ubuntu"));
        assert_eq!(os.version.as_deref(), Some("22.04"));
        assert_eq!(os.full_name.as_deref(), Some("Ubuntu 22.04.3 LTS"));
        // Not provided by os-release.
        assert_eq!(os.kernel_version, None);
        assert_eq!(os.arch, None);
    }

    #[test]
    fn parses_debian_and_ignores_comments() {
        let os = parse_os_release(DEBIAN);
        assert_eq!(os.name.as_deref(), Some("Debian GNU/Linux"));
        assert_eq!(os.version.as_deref(), Some("12"));
        assert_eq!(
            os.full_name.as_deref(),
            Some("Debian GNU/Linux 12 (bookworm)")
        );
    }

    #[test]
    fn empty_file_yields_empty_os() {
        assert_eq!(parse_os_release(""), super::OperatingSystem::default());
    }

    #[test]
    fn unquotes_single_and_double_quotes() {
        let os = parse_os_release("NAME='Arch Linux'\nVERSION_ID=rolling\n");
        assert_eq!(os.name.as_deref(), Some("Arch Linux"));
        assert_eq!(os.version.as_deref(), Some("rolling"));
    }

    #[test]
    fn extracts_timezone_from_symlink_target() {
        use super::parse_timezone_name;
        assert_eq!(
            parse_timezone_name("/usr/share/zoneinfo/Europe/Berlin").as_deref(),
            Some("Europe/Berlin")
        );
        assert_eq!(
            parse_timezone_name("../usr/share/zoneinfo/UTC").as_deref(),
            Some("UTC")
        );
        assert_eq!(parse_timezone_name("/etc/localtime"), None);
    }
}
