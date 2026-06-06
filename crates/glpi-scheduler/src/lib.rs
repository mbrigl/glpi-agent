// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-scheduler` — daemon scheduling for the GLPI Agent Rust workspace.
//!
//! Decides when each target runs. Landing incrementally; currently available:
//!
//! - [`backoff`] — [`Backoff`], the doubling delay applied after network
//!   failures,
//! - [`schedule`] — [`RunSchedule`], next-run tracking with `delaytime` jitter,
//! - [`event`] — [`Event`], the typed agent events (`init`, `runnow`,
//!   `taskrun`, `partial`, `maintenance`, `job`),
//! - [`ipc`] — the framed task-fork IPC protocol,
//! - [`fork`] — the parent/child halves that run a task in a child process over
//!   that protocol,
//! - [`lifecycle`] — the daemon process lifecycle: a [`PidFile`] and background
//!   [`detach`](lifecycle::detach).
//!
//! Targets and the daemon loop follow in later units.

pub mod backoff;
pub mod event;
pub mod fork;
pub mod ipc;
pub mod lifecycle;
pub mod schedule;

pub use backoff::Backoff;
pub use event::{Event, EventKind};
pub use fork::{collect_messages, read_initial_event, TaskWorker, WorkerOutcome, WorkerReporter};
pub use ipc::{read_message, write_message, IpcMessage};
pub use lifecycle::{detach, is_detached_child, PidFile, DETACH_ENV};
pub use schedule::{jitter, RunSchedule};
