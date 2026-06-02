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

    /// Returns `true` if `program` is runnable on the remote host (`command -v`).
    /// A transport/command failure is reported as "not runnable".
    async fn can_run(&mut self, program: &str) -> bool {
        let command = format!(
            "if command -v {} >/dev/null 2>&1; then echo {MARKER}; fi",
            shell_quote(program)
        );
        matches!(self.run(&command).await, Ok(out) if out.contains(MARKER))
    }

    /// Returns `true` if remote `perl` can load `module` (optionally at or above
    /// `min_version`). Used to gate the `perl`-mode enhancements.
    async fn perl_module(&mut self, module: &str, min_version: Option<&str>) -> bool {
        let probe = match min_version {
            Some(version) => format!("exit(${module}::VERSION < {version} ? 1 : 0)"),
            None => "1".to_owned(),
        };
        let command = format!(
            "perl -M{module} -e {} >/dev/null 2>&1 && echo {MARKER}",
            shell_quote(&probe)
        );
        matches!(self.run(&command).await, Ok(out) if out.contains(MARKER))
    }
}

/// Sentinel printed by the capability probes (unlikely to occur otherwise).
const MARKER: &str = "glpi-agent-ok";

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
    programs: std::collections::HashSet<String>,
    perl_modules: std::collections::HashSet<String>,
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

    /// Marks `program` as runnable on the host (drives [`RemoteSession::can_run`]).
    #[must_use]
    pub fn with_program(mut self, program: &str) -> Self {
        self.programs.insert(program.to_owned());
        self
    }

    /// Marks a Perl `module` as loadable (drives [`RemoteSession::perl_module`]).
    #[must_use]
    pub fn with_perl_module(mut self, module: &str) -> Self {
        self.perl_modules.insert(module.to_owned());
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

    async fn can_run(&mut self, program: &str) -> bool {
        self.programs.contains(program)
    }

    async fn perl_module(&mut self, module: &str, _min_version: Option<&str>) -> bool {
        self.perl_modules.contains(module)
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
