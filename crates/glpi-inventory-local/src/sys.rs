// SPDX-License-Identifier: GPL-2.0-only
#![cfg(any(target_os = "windows", target_os = "macos"))]

//! Helpers for the live Windows/macOS inventory collectors.
//!
//! The platform collectors capture the textual or JSON output of a system tool
//! (`system_profiler`, `sysctl`, PowerShell `Get-CimInstance`, …) and feed it to
//! a pure parser. These helpers run the tool and return its stdout; the parsing
//! lives next to each category so it stays unit-testable on any host.

use std::process::Command;

/// Runs `program args…` and returns its stdout when it exits successfully and
/// the output is non-empty; otherwise `None`.
#[must_use]
pub(crate) fn output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    (!text.trim().is_empty()).then_some(text)
}

/// Runs a PowerShell `script` non-interactively and returns its stdout (Windows).
///
/// Used for the `Get-CimInstance … | ConvertTo-Json` queries the Windows
/// collectors parse.
#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn powershell(script: &str) -> Option<String> {
    output(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
    )
}
