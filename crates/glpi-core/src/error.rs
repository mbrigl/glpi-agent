// SPDX-License-Identifier: GPL-2.0-only

//! The crate-wide error type.
//!
//! Every fallible operation in the GLPI Agent returns [`Result`], which is an
//! alias for `std::result::Result<T, AgentError>`. [`AgentError`] is a flat
//! enum that the task crates extend through the free-form string variants
//! ([`AgentError::Task`], [`AgentError::Protocol`], …) so that a leaf crate does
//! not need to add its own error type for every failure mode.

use std::result::Result as StdResult;

/// The error type shared by all GLPI Agent crates.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AgentError {
    /// A configuration value was missing, malformed, or contradictory.
    #[error("configuration error: {0}")]
    Config(String),

    /// An underlying I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization of an inventory or protocol message failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A value could not be parsed from its textual representation.
    #[error("parse error: {0}")]
    Parse(String),

    /// The GLPI / FusionInventory protocol was violated by either side.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// An HTTP request or other transport-level operation failed.
    #[error("transport error: {0}")]
    Transport(String),

    /// Authentication against the server or a remote target failed.
    #[error("authentication error: {0}")]
    Auth(String),

    /// A task (inventory, discovery, deploy, …) failed while running.
    #[error("task error: {0}")]
    Task(String),

    /// The requested feature or platform combination is not supported.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

/// Convenience alias used throughout the workspace.
pub type Result<T> = StdResult<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::AgentError;

    #[test]
    fn display_includes_context() {
        let err = AgentError::Config("missing server".to_owned());
        assert_eq!(err.to_string(), "configuration error: missing server");
    }

    #[test]
    fn io_error_converts() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let err: AgentError = io.into();
        assert!(matches!(err, AgentError::Io(_)));
    }
}
