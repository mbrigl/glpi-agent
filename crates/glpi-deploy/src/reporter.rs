// SPDX-License-Identifier: GPL-2.0-only

//! Deploy status reporting.
//!
//! As an order runs, the agent posts status updates back to the server
//! ([`StatusReport`]). After a successful deployment it also triggers a partial
//! software inventory so the server sees what changed — [`POSTRUN_PARTIAL_CATEGORY`]
//! names that category. The actual HTTP send sits behind the [`Reporter`] seam.

use async_trait::async_trait;
use glpi_core::error::Result;
use serde::Serialize;

/// The inventory category re-collected after a successful deployment.
pub const POSTRUN_PARTIAL_CATEGORY: &str = "software";

/// The lifecycle status of an order or step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    /// The step is in progress.
    Running,
    /// The step succeeded.
    Ok,
    /// The step failed.
    Ko,
}

/// A status update posted to the server for one order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusReport {
    /// The agent's machine id.
    pub machineid: String,
    /// The order's uuid.
    pub uuid: String,
    /// The current status.
    pub status: StepStatus,
    /// Human-readable log lines for this update.
    #[serde(rename = "msg", skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<String>,
}

impl StatusReport {
    /// Builds a report for `uuid` on `machineid`.
    #[must_use]
    pub fn new(machineid: impl Into<String>, uuid: impl Into<String>, status: StepStatus) -> Self {
        Self {
            machineid: machineid.into(),
            uuid: uuid.into(),
            status,
            messages: Vec::new(),
        }
    }

    /// Adds a log line.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.messages.push(message.into());
        self
    }
}

/// Sends [`StatusReport`]s to the server.
#[async_trait]
pub trait Reporter: Send + Sync {
    /// Posts one status update.
    ///
    /// # Errors
    ///
    /// Returns an error if the update cannot be delivered.
    async fn report(&self, status: &StatusReport) -> Result<()>;
}

/// A [`Reporter`] that records updates in memory, for tests.
#[derive(Debug, Default)]
pub struct MockReporter {
    sent: std::sync::Mutex<Vec<StatusReport>>,
}

impl MockReporter {
    /// Builds an empty recorder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy of every recorded status update.
    #[must_use]
    pub fn sent(&self) -> Vec<StatusReport> {
        self.sent.lock().expect("reporter lock").clone()
    }
}

#[async_trait]
impl Reporter for MockReporter {
    async fn report(&self, status: &StatusReport) -> Result<()> {
        self.sent
            .lock()
            .expect("reporter lock")
            .push(status.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MockReporter, Reporter, StatusReport, StepStatus};

    #[test]
    fn serializes_to_glpi_shape() {
        let report = StatusReport::new("machine-1", "order-9", StepStatus::Ko)
            .with_message("installer failed");
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["machineid"], "machine-1");
        assert_eq!(value["uuid"], "order-9");
        assert_eq!(value["status"], "ko");
        assert_eq!(value["msg"][0], "installer failed");
    }

    #[test]
    fn empty_messages_are_omitted() {
        let report = StatusReport::new("m", "u", StepStatus::Running);
        let value = serde_json::to_value(&report).unwrap();
        assert!(value.get("msg").is_none());
    }

    #[tokio::test]
    async fn mock_reporter_records_updates() {
        let reporter = MockReporter::new();
        reporter
            .report(&StatusReport::new("m", "u", StepStatus::Ok))
            .await
            .unwrap();
        assert_eq!(reporter.sent().len(), 1);
        assert_eq!(reporter.sent()[0].status, StepStatus::Ok);
    }
}
