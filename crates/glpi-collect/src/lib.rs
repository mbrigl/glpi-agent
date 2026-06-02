// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-collect` — the Collect task v3.0 (Phase 9).
//!
//! Part of the GLPI Agent Rust workspace (v2.0.0).
//!
//! The Collect task runs server-supplied collection jobs on the agent host and
//! returns their results. Four functions are supported: `findFile`,
//! `getFromRegistry`, `getFromWMI` and `runCommand`.
//!
//! Platform-specific access (registry, WMI, shell) sits behind seams
//! ([`RegistryReader`], [`WmiClient`], [`CommandRunner`]) so the dispatch logic
//! is exercised cross-platform; the filesystem `findFile` collector and the
//! SHA-256/SHA-512 checksum filters are pure and portable.
//!
//! # Example
//!
//! ```
//! use glpi_collect::{CollectContext, CollectTask, MockCommandRunner};
//! use glpi_collect::{MockRegistry, MockWmi};
//!
//! let jobs = CollectTask::parse_jobs(
//!     r#"[{"uuid":"1","function":"runCommand","command":"echo hi"}]"#,
//! )
//! .unwrap();
//! let registry = MockRegistry::new();
//! let wmi = MockWmi::new();
//! let command = MockCommandRunner::new().with_output("echo hi", "hi\n");
//! let ctx = CollectContext { registry: &registry, wmi: &wmi, command: &command };
//! let results = CollectTask::run(&jobs, &ctx);
//! assert_eq!(results[0].result, serde_json::json!("hi\n"));
//! ```

pub mod checksum;
pub mod file;
pub mod registry;
pub mod task;
pub mod wmi;

pub use checksum::{file_sha256_hex, file_sha512_hex, sha256_hex, sha512_hex};
pub use file::{find_files, glob_match, FindFilter, FoundFile};
#[cfg(windows)]
pub use registry::WindowsRegistry;
pub use registry::{
    decode_multi_sz, decode_utf16le, MockRegistry, RegistryReader, RegistryValue,
    UnsupportedRegistry,
};
pub use task::{
    CollectContext, CollectFunction, CollectJob, CollectTask, CommandRunner, JobResult,
    MockCommandRunner, ShellCommandRunner,
};
pub use wmi::{MockWmi, UnsupportedWmi, WmiClient, WmiInstance};
