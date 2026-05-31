// SPDX-License-Identifier: GPL-2.0-only

//! The standard-error logging backend.

use super::{Backend, LogLevel};

/// A [`Backend`] that writes each message to standard error as
/// `[<level>] <message>`.
#[derive(Debug, Default, Clone, Copy)]
pub struct StderrBackend;

impl StderrBackend {
    /// Creates a stderr backend.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Backend for StderrBackend {
    fn emit(&self, level: LogLevel, message: &str) {
        eprintln!("[{level}] {message}");
    }
}
