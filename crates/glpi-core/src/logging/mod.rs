// SPDX-License-Identifier: GPL-2.0-only

//! The agent's logging facade.
//!
//! This mirrors the upstream Perl agent's logger: a [`Logger`] fans every
//! message out to one or more [`Backend`]s, filtered by a maximum verbosity
//! [`LogLevel`]. Built-in backends cover the common targets:
//!
//! - [`stderr::StderrBackend`] — write to standard error,
//! - [`file::FileBackend`] — append to a log file,
//! - [`CallbackBackend`] — hand each line to a closure (the "callback API" the
//!   GLPI server uses to capture agent output).
//!
//! A `syslog` backend (`cfg(unix)`) is planned for a later step.

pub mod file;
pub mod stderr;

use std::fmt;

pub use file::FileBackend;
pub use stderr::StderrBackend;

/// Severity / verbosity of a log message.
///
/// Ordering runs from least to most verbose, so a [`Logger`] shows every
/// message whose level is `<=` its configured maximum: at the default
/// [`LogLevel::Info`], errors, warnings and info pass while debug is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    /// A failure that prevents the current operation.
    Error,
    /// Something unexpected that does not stop the operation.
    Warning,
    /// High-level progress information.
    #[default]
    Info,
    /// Developer-oriented detail.
    Debug,
    /// Very verbose tracing (the agent's `--debug --debug` level).
    Debug2,
}

impl LogLevel {
    /// Maps an agent debug verbosity count to a maximum [`LogLevel`].
    ///
    /// Mirrors the upstream agent's `--debug` flag: `0` keeps the default
    /// [`LogLevel::Info`], `1` enables [`LogLevel::Debug`], and `2` or more
    /// enables [`LogLevel::Debug2`].
    #[must_use]
    pub const fn from_verbosity(debug: u8) -> Self {
        match debug {
            0 => Self::Info,
            1 => Self::Debug,
            _ => Self::Debug2,
        }
    }

    /// The lower-case label used in rendered log lines (`error`, `warning`, …).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Debug2 => "debug2",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A sink that renders log messages somewhere.
///
/// Backends must be cheap to call and infallible from the caller's point of
/// view: a failing target (a full disk, a closed pipe) is the backend's own
/// problem to absorb, never the caller's.
pub trait Backend: Send + Sync {
    /// Emits a single already-filtered message.
    fn emit(&self, level: LogLevel, message: &str);
}

/// A [`Backend`] that forwards each message to a closure.
pub struct CallbackBackend<F> {
    callback: F,
}

impl<F> CallbackBackend<F>
where
    F: Fn(LogLevel, &str) + Send + Sync,
{
    /// Wraps `callback` as a backend.
    pub const fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> Backend for CallbackBackend<F>
where
    F: Fn(LogLevel, &str) + Send + Sync,
{
    fn emit(&self, level: LogLevel, message: &str) {
        (self.callback)(level, message);
    }
}

/// Fans log messages out to its backends, filtered by [`LogLevel`].
#[derive(Default)]
pub struct Logger {
    backends: Vec<Box<dyn Backend>>,
    max_level: LogLevel,
}

impl Logger {
    /// Creates an empty logger at the default level ([`LogLevel::Info`]).
    ///
    /// Without any backend a logger silently discards everything; add at least
    /// one with [`Logger::with_backend`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the agent's standard logger: a [`StderrBackend`], plus a
    /// [`FileBackend`] when `logfile` is set, at the level implied by the
    /// `debug` verbosity ([`LogLevel::from_verbosity`]).
    #[must_use]
    pub fn for_agent(debug: u8, logfile: Option<&std::path::Path>) -> Self {
        let mut logger = Self::new()
            .with_max_level(LogLevel::from_verbosity(debug))
            .with_backend(StderrBackend::new());
        if let Some(path) = logfile {
            logger = logger.with_backend(FileBackend::new(path));
        }
        logger
    }

    /// Sets the maximum level that will be emitted.
    #[must_use]
    pub fn with_max_level(mut self, level: LogLevel) -> Self {
        self.max_level = level;
        self
    }

    /// Adds a backend.
    #[must_use]
    pub fn with_backend(mut self, backend: impl Backend + 'static) -> Self {
        self.backends.push(Box::new(backend));
        self
    }

    /// The currently configured maximum level.
    #[must_use]
    pub fn max_level(&self) -> LogLevel {
        self.max_level
    }

    /// Logs `message` at `level`, if `level` passes the verbosity filter.
    pub fn log(&self, level: LogLevel, message: &str) {
        if level <= self.max_level {
            for backend in &self.backends {
                backend.emit(level, message);
            }
        }
    }

    /// Logs at [`LogLevel::Error`].
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    /// Logs at [`LogLevel::Warning`].
    pub fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    /// Logs at [`LogLevel::Info`].
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Logs at [`LogLevel::Debug`].
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Logs at [`LogLevel::Debug2`].
    pub fn debug2(&self, message: &str) {
        self.log(LogLevel::Debug2, message);
    }
}

#[cfg(test)]
mod tests {
    use super::{CallbackBackend, LogLevel, Logger};
    use std::sync::{Arc, Mutex};

    /// Shared store the capturing backend appends `(level, message)` pairs to.
    type Captured = Arc<Mutex<Vec<(LogLevel, String)>>>;

    fn capturing() -> (Captured, CallbackBackend<impl Fn(LogLevel, &str)>) {
        let store: Captured = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&store);
        let backend = CallbackBackend::new(move |level, message: &str| {
            sink.lock().unwrap().push((level, message.to_owned()));
        });
        (store, backend)
    }

    #[test]
    fn level_ordering_is_least_to_most_verbose() {
        assert!(LogLevel::Error < LogLevel::Warning);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Debug2);
    }

    #[test]
    fn from_verbosity_maps_debug_count() {
        assert_eq!(LogLevel::from_verbosity(0), LogLevel::Info);
        assert_eq!(LogLevel::from_verbosity(1), LogLevel::Debug);
        assert_eq!(LogLevel::from_verbosity(2), LogLevel::Debug2);
        assert_eq!(LogLevel::from_verbosity(9), LogLevel::Debug2);
    }

    #[test]
    fn for_agent_sets_level_from_debug() {
        assert_eq!(Logger::for_agent(0, None).max_level(), LogLevel::Info);
        assert_eq!(Logger::for_agent(1, None).max_level(), LogLevel::Debug);
        assert_eq!(Logger::for_agent(5, None).max_level(), LogLevel::Debug2);
    }

    #[test]
    fn filters_messages_above_max_level() {
        let (store, backend) = capturing();
        let logger = Logger::new()
            .with_max_level(LogLevel::Info)
            .with_backend(backend);

        logger.error("boom");
        logger.info("hello");
        logger.debug("noisy");

        let captured = store.lock().unwrap();
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0], (LogLevel::Error, "boom".to_owned()));
        assert_eq!(captured[1], (LogLevel::Info, "hello".to_owned()));
    }

    #[test]
    fn raising_level_lets_debug_through() {
        let (store, backend) = capturing();
        let logger = Logger::new()
            .with_max_level(LogLevel::Debug2)
            .with_backend(backend);

        logger.debug("d1");
        logger.debug2("d2");

        assert_eq!(store.lock().unwrap().len(), 2);
    }

    #[test]
    fn empty_logger_discards_silently() {
        Logger::new().error("nobody listening");
    }
}
