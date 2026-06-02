// SPDX-License-Identifier: GPL-2.0-only

//! The remote-execution abstraction.
//!
//! A [`RemoteSession`] runs a shell command on the target host and returns its
//! standard output, and reads a remote file. This is the single seam the
//! inventory orchestrator drives: the local-inventory parsers are pure and
//! reused verbatim, only the *source* of their input text changes. Concrete
//! sessions (SSH command-line, later russh / WinRM) implement this trait; the
//! [`MockSession`] backs the offline tests.

use std::collections::HashMap;

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};

/// Executes commands and reads files on a remote host.
#[async_trait]
pub trait RemoteSession: Send {
    /// Runs `command` through the remote shell and returns its stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport fails or the command exits non-zero.
    async fn run(&mut self, command: &str) -> Result<String>;

    /// Reads a remote file. The default implementation `cat`s it through
    /// [`run`](Self::run); transports with native file access (SFTP) override
    /// this.
    ///
    /// # Errors
    ///
    /// Propagates [`run`](Self::run) failures.
    async fn read_file(&mut self, path: &str) -> Result<String> {
        self.run(&format!("cat -- {}", shell_quote(path))).await
    }
}

/// Single-quotes a string for safe use as one POSIX shell word.
#[must_use]
pub fn shell_quote(s: &str) -> String {
    // Wrap in single quotes; an embedded single quote becomes '\'' .
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// An in-memory [`RemoteSession`] for tests: command/file text is looked up in
/// fixed maps, and an unmapped key yields a "command not found"-style error
/// (so the orchestrator's best-effort handling can be exercised).
#[derive(Debug, Default, Clone)]
pub struct MockSession {
    commands: HashMap<String, String>,
    files: HashMap<String, String>,
}

impl MockSession {
    /// Creates an empty mock session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `output` as the stdout of `command`.
    #[must_use]
    pub fn with_command(mut self, command: &str, output: &str) -> Self {
        self.commands.insert(command.to_owned(), output.to_owned());
        self
    }

    /// Registers `contents` as the body of the file at `path`.
    #[must_use]
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.files.insert(path.to_owned(), contents.to_owned());
        self
    }
}

#[async_trait]
impl RemoteSession for MockSession {
    async fn run(&mut self, command: &str) -> Result<String> {
        self.commands
            .get(command)
            .cloned()
            .ok_or_else(|| AgentError::Task(format!("mock: no output for command {command:?}")))
    }

    async fn read_file(&mut self, path: &str) -> Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| AgentError::Task(format!("mock: no such file {path:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{shell_quote, MockSession, RemoteSession};

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("/etc/os-release"), "'/etc/os-release'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[tokio::test]
    async fn mock_serves_commands_and_files_and_errors_otherwise() {
        let mut session = MockSession::new()
            .with_command("uname -r", "6.1.0\n")
            .with_file("/etc/hostname", "host1\n");
        assert_eq!(session.run("uname -r").await.unwrap(), "6.1.0\n");
        assert_eq!(session.read_file("/etc/hostname").await.unwrap(), "host1\n");
        assert!(session.run("missing").await.is_err());
        assert!(session.read_file("/nope").await.is_err());
    }
}
