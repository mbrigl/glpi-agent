// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-deploy` — the Deploy task v3.5 (Phase 9).
//!
//! Part of the GLPI Agent Rust workspace.
//!
//! The Deploy task applies a server order: evaluate preconditions, fetch and
//! verify the associated files (multipart, SHA-512), run the actions
//! (commands judged by return-code/output checks, plus move/mkdir/delete) and
//! report status — triggering a partial software inventory on success.
//!
//! # Architecture
//!
//! - [`checks`] — the [`CheckProcessor`] and its conditions, including
//!   `fileSHA512` / `fileSHA512mismatch`.
//! - [`checksum`] — SHA-512 hashing with case-insensitive comparison.
//! - [`downloader`] — associated-file part fetching ([`PartFetcher`]) and
//!   SHA-512-verified assembly.
//! - [`p2p`] — peer-mirror candidate enumeration that never includes network or
//!   broadcast addresses.
//! - [`executor`] — command execution with return-code and output matching.
//! - [`reporter`] — status reporting and the post-run partial inventory.
//! - [`task`] — the [`DeployOrder`] model and the [`DeployTask`] orchestration.
//!
//! All I/O (filesystem, command, download, reporting) sits behind seams, so the
//! whole flow is tested offline.

pub mod checks;
pub mod checksum;
pub mod downloader;
pub mod executor;
pub mod p2p;
pub mod reporter;
pub mod task;

pub use checks::{
    Check, CheckEnv, CheckKind, CheckProcessor, CheckReport, MockEnv, OnFailure, RealEnv,
};
pub use checksum::{file_sha512_hex, sha512_hex, sha512_matches};
pub use downloader::{assemble, AssociatedFile, HttpPartFetcher, MockPartFetcher, PartFetcher};
pub use executor::{
    run_action, ActionOutcome, CommandAction, CommandOutput, CommandRunner, MockCommandRunner,
    RetCheck, SystemCommandRunner,
};
pub use p2p::peer_candidates;
pub use reporter::{MockReporter, Reporter, StatusReport, StepStatus, POSTRUN_PARTIAL_CATEGORY};
pub use task::{ActionResult, DeployAction, DeployContext, DeployOrder, DeployReport, DeployTask};
