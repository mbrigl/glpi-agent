// SPDX-License-Identifier: GPL-2.0-only

//! The Deploy check processor — preconditions evaluated before an order runs.
//!
//! Each [`Check`] tests one condition (file presence, size, free space, SHA-512
//! match or mismatch) and carries a `return` policy describing what to do when
//! it fails. [`CheckProcessor::evaluate`] runs them all and decides whether the
//! order may proceed.
//!
//! Filesystem access goes through the [`CheckEnv`] seam so the processor is
//! tested both against real temp files ([`RealEnv`]) and in memory
//! ([`MockEnv`]); free-space queries — which std cannot answer portably — are
//! supplied through the same seam.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::checksum::{file_sha512_hex, sha512_matches};

/// What to do when a check fails. Mirrors GLPI's `return` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    /// Abort the order (the default).
    #[default]
    Ko,
    /// Record a warning but continue.
    Warning,
    /// Record an informational note and continue.
    Info,
    /// Ignore the failure entirely.
    Ignore,
}

/// A single precondition.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Check {
    /// The condition to test.
    #[serde(flatten)]
    pub kind: CheckKind,
    /// What to do on failure (defaults to [`OnFailure::Ko`]).
    #[serde(rename = "return", default)]
    pub on_failure: OnFailure,
}

/// The supported check conditions, tagged by GLPI's `type` field.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum CheckKind {
    /// The path must exist.
    #[serde(rename = "fileExists")]
    FileExists {
        /// Path that must exist.
        path: String,
    },
    /// The path must not exist.
    #[serde(rename = "fileMissing")]
    FileMissing {
        /// Path that must be absent.
        path: String,
    },
    /// The path must exist and be a directory.
    #[serde(rename = "directoryExists")]
    DirectoryExists {
        /// Directory that must exist.
        path: String,
    },
    /// The file size must be greater than `value` bytes.
    #[serde(rename = "fileSizeGreater")]
    FileSizeGreater {
        /// File to size.
        path: String,
        /// Minimum (exclusive) size in bytes.
        value: u64,
    },
    /// At least `value` mebibytes must be free on the path's filesystem.
    #[serde(rename = "freespaceGreater")]
    FreespaceGreater {
        /// Path whose filesystem is measured.
        path: String,
        /// Minimum (exclusive) free space in mebibytes.
        value: u64,
    },
    /// The file must have this SHA-512.
    #[serde(rename = "fileSHA512")]
    FileSha512 {
        /// File to digest.
        path: String,
        /// Expected lower/upper-case hex SHA-512.
        value: String,
    },
    /// The file must *not* have this SHA-512 (absent counts as a mismatch).
    #[serde(rename = "fileSHA512mismatch")]
    FileSha512Mismatch {
        /// File to digest.
        path: String,
        /// SHA-512 the file must not have.
        value: String,
    },
}

/// The result of one evaluated check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    /// Whether the condition held.
    pub passed: bool,
    /// A human-readable explanation when it did not.
    pub message: Option<String>,
    /// The failure policy that applied.
    pub on_failure: OnFailure,
}

/// The outcome of evaluating every check in an order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    /// `true` if the order may run (no `ko` check failed).
    pub proceed: bool,
    /// Per-check results, in order.
    pub results: Vec<CheckResult>,
}

/// Filesystem queries the checks need; injected so it can be mocked.
pub trait CheckEnv {
    /// Whether `path` exists.
    fn exists(&self, path: &str) -> bool;
    /// Whether `path` is a directory.
    fn is_dir(&self, path: &str) -> bool;
    /// The size of the file at `path`, if it is a readable file.
    fn file_size(&self, path: &str) -> Option<u64>;
    /// The SHA-512 of the file at `path`, if readable.
    fn file_sha512(&self, path: &str) -> Option<String>;
    /// Free space in mebibytes on the filesystem holding `path`, if known.
    fn free_space_mib(&self, path: &str) -> Option<u64>;
}

/// The live environment backed by `std::fs`.
///
/// Free space is reported as unknown (std has no portable query), so
/// `freespaceGreater` is treated as best-effort and passes when it cannot be
/// determined.
#[derive(Debug, Default, Clone)]
pub struct RealEnv;

impl CheckEnv for RealEnv {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }
    fn file_size(&self, path: &str) -> Option<u64> {
        std::fs::metadata(path)
            .ok()
            .filter(|m| m.is_file())
            .map(|m| m.len())
    }
    fn file_sha512(&self, path: &str) -> Option<String> {
        file_sha512_hex(Path::new(path)).ok()
    }
    fn free_space_mib(&self, _path: &str) -> Option<u64> {
        None
    }
}

/// The check processor.
#[derive(Debug, Default, Clone)]
pub struct CheckProcessor;

impl CheckProcessor {
    /// Evaluates `checks` against `env`. The order may proceed unless a check
    /// fails with [`OnFailure::Ko`].
    #[must_use]
    pub fn evaluate(checks: &[Check], env: &dyn CheckEnv) -> CheckReport {
        let mut proceed = true;
        let mut results = Vec::with_capacity(checks.len());
        for check in checks {
            let (passed, message) = evaluate_one(&check.kind, env);
            if !passed && check.on_failure == OnFailure::Ko {
                proceed = false;
            }
            results.push(CheckResult {
                passed,
                message,
                on_failure: check.on_failure,
            });
        }
        CheckReport { proceed, results }
    }
}

/// Evaluates a single check, returning `(passed, failure_message)`.
fn evaluate_one(kind: &CheckKind, env: &dyn CheckEnv) -> (bool, Option<String>) {
    match kind {
        CheckKind::FileExists { path } => {
            ok_or(env.exists(path), || format!("{path} does not exist"))
        }
        CheckKind::FileMissing { path } => ok_or(!env.exists(path), || format!("{path} exists")),
        CheckKind::DirectoryExists { path } => {
            ok_or(env.is_dir(path), || format!("{path} is not a directory"))
        }
        CheckKind::FileSizeGreater { path, value } => {
            let size = env.file_size(path);
            ok_or(size.is_some_and(|s| s > *value), || {
                format!("{path} size {size:?} not greater than {value}")
            })
        }
        CheckKind::FreespaceGreater { path, value } => match env.free_space_mib(path) {
            // Unknown free space is treated as satisfied (best-effort).
            None => (true, None),
            Some(free) => ok_or(free > *value, || {
                format!("free space {free} MiB on {path} not greater than {value}")
            }),
        },
        CheckKind::FileSha512 { path, value } => {
            let actual = env.file_sha512(path);
            ok_or(
                actual.as_deref().is_some_and(|a| sha512_matches(a, value)),
                || format!("{path} SHA-512 does not match"),
            )
        }
        CheckKind::FileSha512Mismatch { path, value } => {
            // The mismatch check passes when the file is absent or differs.
            let actual = env.file_sha512(path);
            ok_or(
                actual.as_deref().is_none_or(|a| !sha512_matches(a, value)),
                || format!("{path} SHA-512 unexpectedly matches"),
            )
        }
    }
}

/// Helper: `(true, None)` when `passed`, else `(false, Some(message()))`.
fn ok_or(passed: bool, message: impl FnOnce() -> String) -> (bool, Option<String>) {
    if passed {
        (true, None)
    } else {
        (false, Some(message()))
    }
}

/// An in-memory [`CheckEnv`] for tests.
#[derive(Debug, Default, Clone)]
pub struct MockEnv {
    /// path → (is_dir, size, sha512)
    files: BTreeMap<String, (bool, u64, Option<String>)>,
    free_space: BTreeMap<String, u64>,
}

impl MockEnv {
    /// Builds an empty mock environment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Registers a regular file with a size and optional SHA-512.
    #[must_use]
    pub fn with_file(mut self, path: &str, size: u64, sha512: Option<&str>) -> Self {
        self.files
            .insert(path.to_owned(), (false, size, sha512.map(str::to_owned)));
        self
    }
    /// Registers a directory.
    #[must_use]
    pub fn with_dir(mut self, path: &str) -> Self {
        self.files.insert(path.to_owned(), (true, 0, None));
        self
    }
    /// Sets the free space (MiB) reported for a path.
    #[must_use]
    pub fn with_free_space(mut self, path: &str, mib: u64) -> Self {
        self.free_space.insert(path.to_owned(), mib);
        self
    }
}

impl CheckEnv for MockEnv {
    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
    fn is_dir(&self, path: &str) -> bool {
        self.files.get(path).is_some_and(|(is_dir, ..)| *is_dir)
    }
    fn file_size(&self, path: &str) -> Option<u64> {
        self.files
            .get(path)
            .filter(|(is_dir, ..)| !is_dir)
            .map(|(_, size, _)| *size)
    }
    fn file_sha512(&self, path: &str) -> Option<String> {
        self.files.get(path).and_then(|(_, _, sha)| sha.clone())
    }
    fn free_space_mib(&self, path: &str) -> Option<u64> {
        self.free_space.get(path).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{Check, CheckProcessor, MockEnv, OnFailure, RealEnv};
    use crate::checksum::sha512_hex;

    #[test]
    fn file_sha512_check_against_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        std::fs::write(&path, b"installer-bytes").unwrap();
        let digest = sha512_hex(b"installer-bytes");
        let path_str = path.to_string_lossy().into_owned();

        // Matching digest passes; the mismatch check therefore fails.
        let checks = vec![
            Check {
                kind: super::CheckKind::FileSha512 {
                    path: path_str.clone(),
                    value: digest.to_ascii_uppercase(),
                },
                on_failure: OnFailure::Ko,
            },
            Check {
                kind: super::CheckKind::FileSha512Mismatch {
                    path: path_str,
                    value: digest,
                },
                on_failure: OnFailure::Ko,
            },
        ];
        let report = CheckProcessor::evaluate(&checks, &RealEnv);
        assert!(report.results[0].passed, "sha512 should match");
        assert!(!report.results[1].passed, "mismatch must fail when equal");
        assert!(!report.proceed);
    }

    #[test]
    fn sha512_mismatch_passes_when_file_absent() {
        let checks = vec![Check {
            kind: super::CheckKind::FileSha512Mismatch {
                path: "/nonexistent".to_owned(),
                value: "abc".to_owned(),
            },
            on_failure: OnFailure::Ko,
        }];
        let report = CheckProcessor::evaluate(&checks, &MockEnv::new());
        assert!(report.results[0].passed);
        assert!(report.proceed);
    }

    #[test]
    fn ko_blocks_but_warning_proceeds() {
        let env = MockEnv::new();
        let ko = vec![Check {
            kind: super::CheckKind::FileExists {
                path: "/x".to_owned(),
            },
            on_failure: OnFailure::Ko,
        }];
        assert!(!CheckProcessor::evaluate(&ko, &env).proceed);

        let warn = vec![Check {
            kind: super::CheckKind::FileExists {
                path: "/x".to_owned(),
            },
            on_failure: OnFailure::Warning,
        }];
        let report = CheckProcessor::evaluate(&warn, &env);
        assert!(report.proceed);
        assert!(!report.results[0].passed);
    }

    #[test]
    fn freespace_and_size_checks() {
        let env = MockEnv::new()
            .with_file("/big", 4096, None)
            .with_free_space("/data", 500);
        let checks: Vec<Check> = serde_json::from_str(
            r#"[
                {"type":"fileSizeGreater","path":"/big","value":1024},
                {"type":"freespaceGreater","path":"/data","value":1000,"return":"warning"}
            ]"#,
        )
        .unwrap();
        let report = CheckProcessor::evaluate(&checks, &env);
        assert!(report.results[0].passed);
        assert!(!report.results[1].passed);
        assert!(report.proceed); // the freespace failure is only a warning
    }

    #[test]
    fn deserializes_default_return_as_ko() {
        let checks: Vec<Check> =
            serde_json::from_str(r#"[{"type":"fileExists","path":"/a"}]"#).unwrap();
        assert_eq!(checks[0].on_failure, OnFailure::Ko);
    }
}
