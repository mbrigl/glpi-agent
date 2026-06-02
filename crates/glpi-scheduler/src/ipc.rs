// SPDX-License-Identifier: GPL-2.0-only

//! Task-fork IPC protocol.
//!
//! To isolate a task run, the daemon executes it in a forked child (Unix:
//! `fork()` + pipe; Windows: `CreateProcess` + named pipe — in this port a
//! re-exec'd child process with piped stdio). Parent and child speak the
//! framed [`IpcMessage`] protocol over any byte stream: the parent sends the
//! [`Event`] to run, the child streams back log lines, progress and the
//! produced result, then a terminal [`IpcMessage::Done`].
//!
//! Each frame is a 4-byte big-endian length followed by that many bytes of
//! JSON, so an arbitrarily large message (e.g. a full inventory, or the
//! `ssl-rename` event payload) crosses the channel without a line-length limit.
//! The framing is transport-agnostic ([`write_message`] / [`read_message`] take
//! any [`AsyncWrite`] / [`AsyncRead`]), so it is exercised end-to-end over an
//! in-memory pipe in tests; wiring it to a child process's stdio is the only
//! environment-specific part.

use glpi_core::error::{AgentError, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::event::Event;

/// The largest frame accepted, guarding against a corrupt length prefix.
const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;

/// A message exchanged between the daemon (parent) and a forked task (child).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    /// Parent → child: the event to run (or child → parent for an internal
    /// event such as an SSL certificate rename).
    Event(Event),
    /// Child → parent: a log line.
    Log {
        /// Log level (`error`, `info`, `debug`, …).
        level: String,
        /// The message text.
        message: String,
    },
    /// Child → parent: task progress.
    Progress {
        /// Task name.
        task: String,
        /// Completion percentage (0–100).
        percent: u8,
    },
    /// Child → parent: a produced result (e.g. an inventory document); may be
    /// large.
    Result {
        /// MIME / format hint (`application/json`, `application/xml`, …).
        content_type: String,
        /// The raw payload bytes.
        data: Vec<u8>,
    },
    /// Child → parent: terminal message ending the run.
    Done {
        /// Whether the task succeeded.
        success: bool,
        /// Optional human-readable detail.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

/// Writes one length-prefixed [`IpcMessage`] frame to `writer`.
///
/// # Errors
///
/// Returns [`AgentError::Json`] on serialization failure, [`AgentError::Task`]
/// for an over-size frame, or [`AgentError::Io`] on a write failure.
pub async fn write_message<W>(writer: &mut W, message: &IpcMessage) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(message)?;
    let len = u32::try_from(body.len())
        .map_err(|_| AgentError::Task("IPC frame too large".to_owned()))?;
    writer
        .write_all(&len.to_be_bytes())
        .await
        .map_err(AgentError::Io)?;
    writer.write_all(&body).await.map_err(AgentError::Io)?;
    writer.flush().await.map_err(AgentError::Io)?;
    Ok(())
}

/// Reads one [`IpcMessage`] frame from `reader`, or `None` at clean EOF.
///
/// # Errors
///
/// [`AgentError::Task`] for an over-size or truncated frame, [`AgentError::Io`]
/// on a read failure, or [`AgentError::Json`] on a malformed payload.
pub async fn read_message<R>(reader: &mut R) -> Result<Option<IpcMessage>>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        // Clean EOF before any byte of the next frame.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(AgentError::Io(e)),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(AgentError::Task(format!(
            "IPC frame of {len} bytes exceeds limit"
        )));
    }
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| AgentError::Task(format!("truncated IPC frame: {e}")))?;
    Ok(Some(serde_json::from_slice(&body)?))
}

#[cfg(test)]
mod tests {
    use super::{read_message, write_message, IpcMessage};
    use crate::event::Event;

    #[tokio::test]
    async fn frames_round_trip_over_a_pipe() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let messages = vec![
            IpcMessage::Event(Event::task_run("inventory", 0, false, Some(true))),
            IpcMessage::Log {
                level: "info".to_owned(),
                message: "started".to_owned(),
            },
            IpcMessage::Progress {
                task: "inventory".to_owned(),
                percent: 50,
            },
            IpcMessage::Done {
                success: true,
                message: None,
            },
        ];
        let sent = messages.clone();
        let writer = tokio::spawn(async move {
            for message in &sent {
                write_message(&mut a, message).await.unwrap();
            }
            drop(a); // EOF
        });

        let mut received = Vec::new();
        while let Some(message) = read_message(&mut b).await.unwrap() {
            received.push(message);
        }
        writer.await.unwrap();
        assert_eq!(received, messages);
    }

    #[tokio::test]
    async fn large_result_crosses_without_a_line_limit() {
        // A multi-megabyte payload (bigger than the pipe buffer) must survive.
        let data = vec![0xABu8; 3 * 1024 * 1024];
        let message = IpcMessage::Result {
            content_type: "application/json".to_owned(),
            data: data.clone(),
        };
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        let sent = message.clone();
        let writer = tokio::spawn(async move {
            write_message(&mut a, &sent).await.unwrap();
            drop(a);
        });
        let received = read_message(&mut b).await.unwrap().unwrap();
        writer.await.unwrap();
        assert_eq!(received, message);
    }

    #[tokio::test]
    async fn clean_eof_yields_none() {
        let (a, mut b) = tokio::io::duplex(64);
        drop(a);
        assert!(read_message(&mut b).await.unwrap().is_none());
    }

    #[test]
    fn message_tag_is_snake_case() {
        let json = serde_json::to_string(&IpcMessage::Done {
            success: false,
            message: Some("boom".to_owned()),
        })
        .unwrap();
        assert!(json.contains("\"type\":\"done\""));
    }
}
