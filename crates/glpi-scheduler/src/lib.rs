// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-scheduler` — daemon scheduling for the GLPI Agent Rust workspace
//! (v2.0.0).
//!
//! Decides when each target runs. Landing incrementally; currently available:
//!
//! - [`backoff`] — [`Backoff`], the doubling delay applied after network
//!   failures,
//! - [`schedule`] — [`RunSchedule`], next-run tracking with `delaytime` jitter.
//!
//! Targets, the event system and the daemon loop follow in later units.

pub mod backoff;
pub mod schedule;

pub use backoff::Backoff;
pub use schedule::{jitter, RunSchedule};
