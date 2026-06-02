// SPDX-License-Identifier: GPL-2.0-only

//! The Collect task: run server-supplied collection jobs and return results.
//!
//! GLPI's Collect task hands the agent a list of jobs, each naming a `function`
//! (`findFile`, `getFromRegistry`, `getFromWMI`, `runCommand`) and its
//! parameters. [`CollectTask::run`] dispatches each job against the injected
//! [`CollectContext`] seams and produces one [`JobResult`] per job, ready to be
//! posted back.

use std::collections::BTreeMap;

use glpi_core::error::{AgentError, Result};
use serde::{Deserialize, Serialize};

use crate::file::{find_files, FindFilter};
use crate::registry::RegistryReader;
use crate::wmi::WmiClient;

/// A single Collect job: a stable `uuid` plus the function to run.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CollectJob {
    /// Server-assigned job identifier echoed back in the result.
    pub uuid: String,
    /// The collection function and its parameters.
    #[serde(flatten)]
    pub function: CollectFunction,
}

/// A Collect function, tagged by the GLPI `function` field.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "function")]
pub enum CollectFunction {
    /// Find files under a directory matching a filter.
    #[serde(rename = "findFile")]
    FindFile {
        /// Directory to search.
        dir: String,
        /// Recurse into sub-directories.
        #[serde(default)]
        recursive: bool,
        /// Maximum number of matches (`0` = unlimited).
        #[serde(default)]
        limit: usize,
        /// Match filter.
        #[serde(default)]
        filter: FindFilter,
    },
    /// Read the values of a Windows registry key.
    #[serde(rename = "getFromRegistry")]
    GetFromRegistry {
        /// The registry key path.
        key: String,
    },
    /// Query WMI instances.
    #[serde(rename = "getFromWMI")]
    GetFromWmi {
        /// WMI class name.
        class: String,
        /// Properties to project (empty = all).
        #[serde(default)]
        properties: Vec<String>,
    },
    /// Run a shell command and capture its stdout.
    #[serde(rename = "runCommand")]
    RunCommand {
        /// The command line to run.
        command: String,
    },
}

/// The result of running one job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobResult {
    /// The job's `uuid`.
    pub uuid: String,
    /// The collected data (job-specific shape), or `null` on error.
    pub result: serde_json::Value,
    /// Error message when the job failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Runs a shell command and returns its stdout.
pub trait CommandRunner {
    /// Runs `command` and returns its captured stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if the command cannot be spawned or exits abnormally.
    fn run(&self, command: &str) -> Result<String>;
}

/// A [`CommandRunner`] that executes through the system shell.
#[derive(Debug, Default, Clone)]
pub struct ShellCommandRunner;

impl CommandRunner for ShellCommandRunner {
    fn run(&self, command: &str) -> Result<String> {
        let output = if cfg!(windows) {
            std::process::Command::new("cmd")
                .args(["/C", command])
                .output()
        } else {
            std::process::Command::new("sh")
                .args(["-c", command])
                .output()
        }
        .map_err(|e| AgentError::Task(format!("spawning command {command:?}: {e}")))?;
        if !output.status.success() {
            return Err(AgentError::Task(format!(
                "command {command:?} exited with {}",
                output.status
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// A [`CommandRunner`] with canned outputs for tests.
#[derive(Debug, Default, Clone)]
pub struct MockCommandRunner {
    outputs: BTreeMap<String, String>,
}

impl MockCommandRunner {
    /// Builds an empty mock runner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `output` for an exact `command`.
    #[must_use]
    pub fn with_output(mut self, command: &str, output: &str) -> Self {
        self.outputs.insert(command.to_owned(), output.to_owned());
        self
    }
}

impl CommandRunner for MockCommandRunner {
    fn run(&self, command: &str) -> Result<String> {
        self.outputs
            .get(command)
            .cloned()
            .ok_or_else(|| AgentError::Task(format!("no mock output for command {command:?}")))
    }
}

/// The platform seams a Collect run dispatches against.
pub struct CollectContext<'a> {
    /// Windows registry reader.
    pub registry: &'a dyn RegistryReader,
    /// WMI client.
    pub wmi: &'a dyn WmiClient,
    /// Shell command runner.
    pub command: &'a dyn CommandRunner,
}

/// The Collect task.
#[derive(Debug, Default, Clone)]
pub struct CollectTask;

impl CollectTask {
    /// Parses a server job list (a JSON array) into [`CollectJob`]s.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Json`] if the payload is malformed.
    pub fn parse_jobs(json: &str) -> Result<Vec<CollectJob>> {
        Ok(serde_json::from_str(json)?)
    }

    /// Runs every job against `ctx`, collecting one [`JobResult`] each. A job
    /// that fails is reported with its `error` set rather than aborting the run.
    #[must_use]
    pub fn run(jobs: &[CollectJob], ctx: &CollectContext<'_>) -> Vec<JobResult> {
        jobs.iter()
            .map(|job| match run_job(&job.function, ctx) {
                Ok(result) => JobResult {
                    uuid: job.uuid.clone(),
                    result,
                    error: None,
                },
                Err(err) => {
                    tracing::warn!(uuid = %job.uuid, error = %err, "collect job failed");
                    JobResult {
                        uuid: job.uuid.clone(),
                        result: serde_json::Value::Null,
                        error: Some(err.to_string()),
                    }
                }
            })
            .collect()
    }
}

/// Dispatches a single function to its seam and returns its JSON result.
fn run_job(function: &CollectFunction, ctx: &CollectContext<'_>) -> Result<serde_json::Value> {
    match function {
        CollectFunction::FindFile {
            dir,
            recursive,
            limit,
            filter,
        } => {
            let found = find_files(std::path::Path::new(dir), *recursive, *limit, filter);
            Ok(serde_json::to_value(found)?)
        }
        CollectFunction::GetFromRegistry { key } => {
            let values = ctx.registry.read_values(key)?;
            let rendered: BTreeMap<String, String> = values
                .into_iter()
                .map(|(name, value)| (name, value.to_glpi_string()))
                .collect();
            Ok(serde_json::to_value(rendered)?)
        }
        CollectFunction::GetFromWmi { class, properties } => {
            let rows = ctx.wmi.query(class, properties)?;
            Ok(serde_json::to_value(rows)?)
        }
        CollectFunction::RunCommand { command } => {
            Ok(serde_json::Value::String(ctx.command.run(command)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectContext, CollectTask, MockCommandRunner};
    use crate::registry::{MockRegistry, RegistryValue};
    use crate::wmi::MockWmi;
    use std::fs;

    #[test]
    fn parses_a_mixed_job_list() {
        let json = r#"[
            {"uuid":"1","function":"runCommand","command":"echo hi"},
            {"uuid":"2","function":"findFile","dir":"/tmp","recursive":true,"filter":{"name":"*.log"}},
            {"uuid":"3","function":"getFromRegistry","key":"HKLM/X"},
            {"uuid":"4","function":"getFromWMI","class":"Win32_Service","properties":["Name"]}
        ]"#;
        let jobs = CollectTask::parse_jobs(json).unwrap();
        assert_eq!(jobs.len(), 4);
        assert_eq!(jobs[0].uuid, "1");
    }

    #[test]
    fn runs_every_job_kind() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("app.log"), b"x").unwrap();

        // Build via serde_json so the directory path is escaped correctly on
        // every platform (Windows paths contain backslashes).
        let json = serde_json::json!([
            {"uuid": "cmd", "function": "runCommand", "command": "echo hi"},
            {"uuid": "file", "function": "findFile", "dir": dir.path().to_str().unwrap(),
             "filter": {"name": "*.log"}},
            {"uuid": "reg", "function": "getFromRegistry", "key": "HKLM/App"},
            {"uuid": "wmi", "function": "getFromWMI", "class": "Win32_Service"}
        ])
        .to_string();
        let jobs = CollectTask::parse_jobs(&json).unwrap();

        let registry = MockRegistry::new().with_value(
            "HKLM/App",
            "Version",
            RegistryValue::MultiString(vec!["1".to_owned(), "2".to_owned()]),
        );
        let wmi = MockWmi::new().with_instance(
            "Win32_Service",
            [("Name".to_owned(), "wuauserv".to_owned())]
                .into_iter()
                .collect(),
        );
        let command = MockCommandRunner::new().with_output("echo hi", "hi\n");
        let ctx = CollectContext {
            registry: &registry,
            wmi: &wmi,
            command: &command,
        };

        let results = CollectTask::run(&jobs, &ctx);
        assert_eq!(results.len(), 4);

        let by_uuid = |id: &str| results.iter().find(|r| r.uuid == id).unwrap();
        assert_eq!(by_uuid("cmd").result, serde_json::json!("hi\n"));
        assert_eq!(by_uuid("file").result.as_array().unwrap().len(), 1);
        assert_eq!(by_uuid("reg").result["Version"], serde_json::json!("1\n2"));
        assert_eq!(
            by_uuid("wmi").result.as_array().unwrap()[0]["Name"],
            "wuauserv"
        );
        assert!(results.iter().all(|r| r.error.is_none()));
    }

    #[test]
    fn failing_job_is_reported_not_fatal() {
        let json = r#"[{"uuid":"x","function":"runCommand","command":"unmapped"}]"#;
        let jobs = CollectTask::parse_jobs(json).unwrap();
        let registry = MockRegistry::new();
        let wmi = MockWmi::new();
        let command = MockCommandRunner::new();
        let ctx = CollectContext {
            registry: &registry,
            wmi: &wmi,
            command: &command,
        };
        let results = CollectTask::run(&jobs, &ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].error.is_some());
        assert!(results[0].result.is_null());
    }
}
