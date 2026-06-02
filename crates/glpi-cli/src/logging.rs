// SPDX-License-Identifier: GPL-2.0-only

//! Logging backends for the agent.
//!
//! Ported from the upstream agent's `--logger` handling: the agent can log to
//! the terminal (`stderr`, the default), to a `logfile`, or to the system log
//! (`syslog`). A detached daemon (see [`crate`]'s `--daemonize`) has no terminal
//! and must use a logfile or syslog to stay observable.
//!
//! Output goes through `tracing-subscriber`'s `fmt` layer with a backend-chosen
//! [`MakeWriter`]: `stderr`/`File` use the built-in writers, while [`SyslogMaker`]
//! sends one datagram per event to the local syslog socket (`/dev/log`).

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::ValueEnum;
use tracing::{Level, Metadata};
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriter};
use tracing_subscriber::EnvFilter;

/// The path of the local syslog datagram socket.
const SYSLOG_SOCKET: &str = "/dev/log";
/// The syslog tag (program name) stamped on every message.
const SYSLOG_TAG: &str = "glpi-agent";

/// Which logging backend to use (`--logger`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum LoggerKind {
    /// Log to the terminal's standard error (default).
    #[default]
    Stderr,
    /// Log to the file given by `--logfile`.
    File,
    /// Log to the system log via the local syslog socket.
    Syslog,
}

/// Installs the global tracing subscriber for the chosen backend.
///
/// The verbosity filter comes from `RUST_LOG` (default `info`). `stderr` keeps
/// ANSI colours; `file` and `syslog` are plain.
///
/// # Errors
///
/// Returns an error if `file` is selected without a `--logfile`, the log file
/// cannot be opened, or the syslog socket cannot be reached.
pub fn init(kind: LoggerKind, logfile: Option<&Path>, facility: &str) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let writer = match kind {
        LoggerKind::Stderr => BoxMakeWriter::new(io::stderr),
        LoggerKind::File => {
            let path = logfile.context("--logger file requires --logfile <PATH>")?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("opening log file {}", path.display()))?;
            BoxMakeWriter::new(file)
        }
        LoggerKind::Syslog => BoxMakeWriter::new(SyslogMaker::connect(facility)?),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(kind == LoggerKind::Stderr)
        .with_writer(writer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("installing the log subscriber: {e}"))
}

/// A [`MakeWriter`] that emits each tracing event as one syslog datagram.
#[derive(Clone, Debug)]
pub struct SyslogMaker {
    socket: Arc<std::os::unix::net::UnixDatagram>,
    facility: u8,
    pid: u32,
}

impl SyslogMaker {
    /// Connects to the local syslog socket (`/dev/log`).
    ///
    /// # Errors
    ///
    /// An error if the socket cannot be created or connected (e.g. no syslog
    /// daemon is running).
    pub fn connect(facility: &str) -> Result<Self> {
        Self::connect_to(Path::new(SYSLOG_SOCKET), facility)
    }

    /// Connects to the syslog datagram socket at `path` (used by tests).
    fn connect_to(path: &Path, facility: &str) -> Result<Self> {
        let socket =
            std::os::unix::net::UnixDatagram::unbound().context("creating the syslog socket")?;
        socket
            .connect(path)
            .with_context(|| format!("connecting to syslog socket {}", path.display()))?;
        Ok(Self {
            socket: Arc::new(socket),
            facility: facility_code(facility),
            pid: process::id(),
        })
    }

    /// Builds a per-event line writer for the given syslog severity.
    fn line(&self, severity: u8) -> SyslogLine {
        SyslogLine {
            socket: self.socket.clone(),
            priority: self.facility * 8 + severity,
            pid: self.pid,
            buffer: Vec::new(),
        }
    }
}

impl<'a> MakeWriter<'a> for SyslogMaker {
    type Writer = SyslogLine;

    fn make_writer(&'a self) -> Self::Writer {
        self.line(severity(Level::INFO))
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        self.line(severity(*meta.level()))
    }
}

/// A single event's worth of bytes, sent as one datagram when dropped.
pub struct SyslogLine {
    socket: Arc<std::os::unix::net::UnixDatagram>,
    priority: u8,
    pid: u32,
    buffer: Vec<u8>,
}

impl Write for SyslogLine {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for SyslogLine {
    fn drop(&mut self) {
        let message = String::from_utf8_lossy(&self.buffer);
        let message = message.trim_end();
        if message.is_empty() {
            return;
        }
        let datagram = syslog_datagram(self.priority, SYSLOG_TAG, self.pid, message);
        // Best-effort: a logging failure must not crash the agent.
        let _ = self.socket.send(datagram.as_bytes());
    }
}

/// Formats an RFC 3164-style syslog line: `<PRI>tag[pid]: message`.
fn syslog_datagram(priority: u8, tag: &str, pid: u32, message: &str) -> String {
    format!("<{priority}>{tag}[{pid}]: {message}")
}

/// Maps a tracing [`Level`] to a syslog severity (RFC 5424).
fn severity(level: Level) -> u8 {
    match level {
        Level::ERROR => 3, // Error
        Level::WARN => 4,  // Warning
        Level::INFO => 6,  // Informational
        Level::DEBUG | Level::TRACE => 7,
    }
}

/// Maps a syslog facility name to its code (RFC 5424); defaults to `user` (1).
fn facility_code(name: &str) -> u8 {
    match name.to_ascii_lowercase().as_str() {
        "kern" => 0,
        "user" => 1,
        "mail" => 2,
        "daemon" => 3,
        "auth" => 4,
        "syslog" => 5,
        "lpr" => 6,
        "news" => 7,
        "uucp" => 8,
        "cron" => 9,
        "authpriv" => 10,
        "ftp" => 11,
        "local0" => 16,
        "local1" => 17,
        "local2" => 18,
        "local3" => 19,
        "local4" => 20,
        "local5" => 21,
        "local6" => 22,
        "local7" => 23,
        _ => 1,
    }
}

/// Validates the logger options without installing a subscriber (so the CLI can
/// reject a bad combination up front and tests can exercise it).
///
/// # Errors
///
/// An error if `file` is selected without a `--logfile`.
pub fn validate(kind: LoggerKind, logfile: Option<&PathBuf>) -> Result<()> {
    if kind == LoggerKind::File && logfile.is_none() {
        anyhow::bail!("--logger file requires --logfile <PATH>");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{facility_code, severity, syslog_datagram, validate, LoggerKind, SyslogMaker};
    use std::io::Write;
    use std::os::unix::net::UnixDatagram;
    use tracing::Level;

    #[test]
    fn severity_and_facility_mappings() {
        assert_eq!(severity(Level::ERROR), 3);
        assert_eq!(severity(Level::WARN), 4);
        assert_eq!(severity(Level::INFO), 6);
        assert_eq!(severity(Level::TRACE), 7);
        assert_eq!(facility_code("user"), 1);
        assert_eq!(facility_code("DAEMON"), 3);
        assert_eq!(facility_code("local0"), 16);
        // Unknown -> user.
        assert_eq!(facility_code("nope"), 1);
    }

    #[test]
    fn datagram_has_priority_tag_and_pid() {
        // local0 (16) + warning (4) -> 132.
        assert_eq!(
            syslog_datagram(132, "glpi-agent", 42, "danger"),
            "<132>glpi-agent[42]: danger"
        );
    }

    #[test]
    fn validate_requires_a_logfile_for_file_logger() {
        assert!(validate(LoggerKind::File, None).is_err());
        assert!(validate(LoggerKind::File, Some(&"/tmp/x.log".into())).is_ok());
        assert!(validate(LoggerKind::Stderr, None).is_ok());
    }

    #[test]
    fn syslog_writer_sends_one_datagram_per_event() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("log");
        let listener = UnixDatagram::bind(&socket_path).unwrap();

        let maker = SyslogMaker::connect_to(&socket_path, "local0").unwrap();
        {
            // A WARN event: local0 (16) * 8 + warning (4) = 132.
            let mut line = maker.line(severity(Level::WARN));
            writeln!(line, "disk almost full").unwrap();
        } // dropped here -> datagram sent

        let mut buf = [0u8; 256];
        let n = listener.recv(&mut buf).unwrap();
        let got = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(got.starts_with("<132>glpi-agent["), "got: {got}");
        assert!(got.ends_with("]: disk almost full"), "got: {got}");
    }
}
