// SPDX-License-Identifier: GPL-2.0-only

//! Task-fork process management.
//!
//! Upstream runs each task in a `fork()`ed child; this port re-execs a child
//! process with piped stdio and speaks the framed [`IpcMessage`] protocol over
//! it (see [`crate::ipc`]). This module has the two halves:
//!
//! * the **parent** ([`TaskWorker`]) spawns a worker command, sends it the
//!   [`Event`] to run, then streams back the worker's log / progress / result
//!   frames until the terminal [`IpcMessage::Done`];
//! * the **child** ([`read_initial_event`] + [`WorkerReporter`]) reads the event
//!   off its stdin and reports progress / results / completion on its stdout.
//!
//! The crate stays task-agnostic: the spawned program (the `glpi-agent` binary's
//! hidden task-worker subcommand) supplies the actual task logic and only uses
//! these helpers for the wire protocol.

use std::process::{ExitStatus, Stdio};

use glpi_core::error::{AgentError, Result};
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::event::Event;
use crate::ipc::{read_message, write_message, IpcMessage};

/// The terminal outcome of a worker run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerOutcome {
    /// Whether the worker reported success in its [`IpcMessage::Done`].
    pub success: bool,
    /// Optional human-readable detail from the worker.
    pub message: Option<String>,
    /// Results the worker produced, as `(content_type, data)` pairs.
    pub results: Vec<(String, Vec<u8>)>,
}

/// A task running in a child process, addressed over framed IPC on its stdio.
pub struct TaskWorker {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl TaskWorker {
    /// Spawns `command` as a task worker (piping its stdin/stdout) and sends the
    /// initial [`Event`] to run.
    ///
    /// # Errors
    ///
    /// [`AgentError::Io`] if the process cannot be spawned, or a protocol error
    /// if its stdio is unavailable or the initial event cannot be written.
    pub async fn spawn(mut command: Command, event: &Event) -> Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(AgentError::Io)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Task("worker stdin unavailable".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Task("worker stdout unavailable".to_owned()))?;
        write_message(&mut stdin, &IpcMessage::Event(event.clone())).await?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
        })
    }

    /// Sends a further message to the worker (e.g. a follow-up event).
    ///
    /// # Errors
    ///
    /// A protocol error if the input has been closed, or an I/O/serialization
    /// error from the write.
    pub async fn send(&mut self, message: &IpcMessage) -> Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| AgentError::Task("worker input is closed".to_owned()))?;
        write_message(stdin, message).await
    }

    /// Closes the worker's input, signalling end-of-events with EOF.
    pub fn close_input(&mut self) {
        self.stdin = None;
    }

    /// Reads the next message from the worker, or `None` at clean EOF.
    ///
    /// # Errors
    ///
    /// A read / framing / decode error from the channel.
    pub async fn next_message(&mut self) -> Result<Option<IpcMessage>> {
        read_message(&mut self.stdout).await
    }

    /// Waits for the worker process to exit.
    ///
    /// # Errors
    ///
    /// [`AgentError::Io`] if the wait fails.
    pub async fn wait(&mut self) -> Result<ExitStatus> {
        self.child.wait().await.map_err(AgentError::Io)
    }

    /// Drains the worker's messages until [`IpcMessage::Done`] (invoking
    /// `on_message` for each), then waits for the process and returns the
    /// outcome. A worker that exits without a `Done` is treated as a failure.
    ///
    /// # Errors
    ///
    /// A channel error while reading, or an I/O error waiting on the process.
    pub async fn run_to_completion<F>(mut self, on_message: F) -> Result<WorkerOutcome>
    where
        F: FnMut(&IpcMessage),
    {
        let outcome = collect_messages(&mut self.stdout, on_message).await?;
        // Reap the child so it does not linger as a zombie.
        let _ = self.wait().await?;
        Ok(outcome)
    }
}

/// Reads framed messages from `reader` until [`IpcMessage::Done`], invoking
/// `on_message` for each and accumulating any results. Factored out of
/// [`TaskWorker::run_to_completion`] so the draining logic is testable over an
/// in-memory pipe.
///
/// # Errors
///
/// A read / framing / decode error from the channel.
pub async fn collect_messages<R, F>(reader: &mut R, mut on_message: F) -> Result<WorkerOutcome>
where
    R: AsyncRead + Unpin,
    F: FnMut(&IpcMessage),
{
    let mut results = Vec::new();
    let mut done: Option<(bool, Option<String>)> = None;
    while let Some(message) = read_message(reader).await? {
        on_message(&message);
        match message {
            IpcMessage::Result { content_type, data } => results.push((content_type, data)),
            IpcMessage::Done { success, message } => {
                done = Some((success, message));
                break;
            }
            IpcMessage::Event(_) | IpcMessage::Log { .. } | IpcMessage::Progress { .. } => {}
        }
    }
    let (success, message) = done.unwrap_or((
        false,
        Some("worker closed the channel without a Done message".to_owned()),
    ));
    Ok(WorkerOutcome {
        success,
        message,
        results,
    })
}

/// Child side: reads the initial [`Event`] the parent sent on `reader`.
///
/// # Errors
///
/// A protocol error if the first frame is not an [`IpcMessage::Event`] or the
/// channel closes first, or a channel decode error.
pub async fn read_initial_event<R>(reader: &mut R) -> Result<Event>
where
    R: AsyncRead + Unpin,
{
    match read_message(reader).await? {
        Some(IpcMessage::Event(event)) => Ok(event),
        Some(other) => Err(AgentError::Task(format!(
            "expected an initial Event from the parent, got {other:?}"
        ))),
        None => Err(AgentError::Task(
            "parent closed the channel before sending an Event".to_owned(),
        )),
    }
}

/// Child side: writes log / progress / result / completion frames back to the
/// parent.
pub struct WorkerReporter<W> {
    writer: W,
}

impl<W> WorkerReporter<W>
where
    W: AsyncWrite + Unpin,
{
    /// Wraps `writer` (typically the worker's stdout).
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Sends a log line.
    ///
    /// # Errors
    ///
    /// An I/O / serialization error from the write.
    pub async fn log(
        &mut self,
        level: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<()> {
        self.emit(IpcMessage::Log {
            level: level.into(),
            message: message.into(),
        })
        .await
    }

    /// Sends a progress update (`percent` in 0–100).
    ///
    /// # Errors
    ///
    /// An I/O / serialization error from the write.
    pub async fn progress(&mut self, task: impl Into<String>, percent: u8) -> Result<()> {
        self.emit(IpcMessage::Progress {
            task: task.into(),
            percent,
        })
        .await
    }

    /// Sends a produced result.
    ///
    /// # Errors
    ///
    /// An I/O / serialization error from the write.
    pub async fn result(&mut self, content_type: impl Into<String>, data: Vec<u8>) -> Result<()> {
        self.emit(IpcMessage::Result {
            content_type: content_type.into(),
            data,
        })
        .await
    }

    /// Sends the terminal [`IpcMessage::Done`] ending the run.
    ///
    /// # Errors
    ///
    /// An I/O / serialization error from the write.
    pub async fn done(&mut self, success: bool, message: Option<String>) -> Result<()> {
        self.emit(IpcMessage::Done { success, message }).await
    }

    async fn emit(&mut self, message: IpcMessage) -> Result<()> {
        write_message(&mut self.writer, &message).await
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_messages, read_initial_event, WorkerReporter};
    use crate::event::Event;
    use crate::ipc::IpcMessage;

    #[tokio::test]
    async fn collect_messages_drains_to_done_and_gathers_results() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        let writer = tokio::spawn(async move {
            let mut reporter = WorkerReporter::new(&mut a);
            reporter.log("info", "starting").await.unwrap();
            reporter.progress("selftest", 50).await.unwrap();
            reporter
                .result("text/plain", b"hello".to_vec())
                .await
                .unwrap();
            reporter.done(true, Some("ok".to_owned())).await.unwrap();
        });

        let mut seen = Vec::new();
        let outcome = collect_messages(&mut b, |m| seen.push(m.clone()))
            .await
            .unwrap();
        writer.await.unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.message.as_deref(), Some("ok"));
        assert_eq!(
            outcome.results,
            vec![("text/plain".to_owned(), b"hello".to_vec())]
        );
        // log + progress + result + done were all observed.
        assert_eq!(seen.len(), 4);
    }

    #[tokio::test]
    async fn missing_done_is_a_failure() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let writer = tokio::spawn(async move {
            let mut reporter = WorkerReporter::new(&mut a);
            reporter.log("info", "started").await.unwrap();
            drop(a); // EOF without a Done
        });
        let outcome = collect_messages(&mut b, |_| {}).await.unwrap();
        writer.await.unwrap();
        assert!(!outcome.success);
        assert!(outcome.message.unwrap().contains("without a Done"));
    }

    #[tokio::test]
    async fn read_initial_event_round_trips() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let event = Event::run_now("netdiscovery", 0, Default::default());
        let sent = event.clone();
        let writer = tokio::spawn(async move {
            crate::ipc::write_message(&mut a, &IpcMessage::Event(sent))
                .await
                .unwrap();
        });
        let received = read_initial_event(&mut b).await.unwrap();
        writer.await.unwrap();
        assert_eq!(received, event);
    }

    #[tokio::test]
    async fn read_initial_event_rejects_a_non_event_first_frame() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            crate::ipc::write_message(
                &mut a,
                &IpcMessage::Log {
                    level: "info".to_owned(),
                    message: "oops".to_owned(),
                },
            )
            .await
            .unwrap();
        });
        assert!(read_initial_event(&mut b).await.is_err());
    }
}
