// SPDX-License-Identifier: GPL-2.0-only

//! The Deploy task: evaluate checks, download files, run actions, report.
//!
//! A [`DeployOrder`] carries preconditions ([`Check`]s), the files to fetch
//! ([`AssociatedFile`]s) and the [`DeployAction`]s to run. [`DeployTask::run`]
//! drives them in order against the injected [`DeployContext`] seams and returns
//! a [`DeployReport`]. On success — and when the order asks for it — the report
//! flags that a partial software inventory should follow.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glpi_core::error::Result;
use serde::Deserialize;

use crate::checks::{Check, CheckEnv, CheckProcessor, CheckReport};
use crate::downloader::{assemble, AssociatedFile, PartFetcher};
use crate::executor::{run_action, CommandAction, CommandRunner};

/// A deployment order from the server.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeployOrder {
    /// Order identifier.
    pub uuid: String,
    /// Files to fetch, keyed by their whole-file SHA-512.
    #[serde(default, rename = "associatedFiles")]
    pub associated_files: BTreeMap<String, AssociatedFile>,
    /// Preconditions evaluated before anything runs.
    #[serde(default)]
    pub checks: Vec<Check>,
    /// Actions run in order once checks pass and files are present.
    #[serde(default)]
    pub actions: Vec<DeployAction>,
    /// Whether to run a partial software inventory after a successful deploy.
    #[serde(default, rename = "runSoftwareInventory")]
    pub run_software_inventory: bool,
}

/// A deployment action (externally tagged, matching GLPI's `{"cmd": {…}}`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployAction {
    /// Run a command judged by its return checks.
    Cmd(CommandAction),
    /// Move/rename a file.
    Move {
        /// Source path.
        from: String,
        /// Destination path.
        to: String,
    },
    /// Create a directory (and parents).
    Mkdir {
        /// Directory path.
        path: String,
    },
    /// Delete a file or directory tree.
    Delete {
        /// Path to remove.
        path: String,
    },
}

/// The result of one action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResult {
    /// Whether the action succeeded.
    pub success: bool,
    /// Failure detail, when unsuccessful.
    pub message: Option<String>,
}

/// The outcome of running an order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployReport {
    /// The order uuid.
    pub uuid: String,
    /// Whether the whole order succeeded.
    pub success: bool,
    /// The precondition evaluation.
    pub check_report: CheckReport,
    /// Per-action results, in order.
    pub action_results: Vec<ActionResult>,
    /// Failure summary, when unsuccessful.
    pub message: Option<String>,
    /// `true` when a partial software inventory should follow.
    pub run_software_inventory: bool,
}

/// The seams a deploy run executes against.
pub struct DeployContext<'a> {
    /// Precondition filesystem queries.
    pub env: &'a dyn CheckEnv,
    /// Command runner for `cmd` actions.
    pub runner: &'a dyn CommandRunner,
    /// Fetcher for associated-file parts.
    pub fetcher: &'a dyn PartFetcher,
    /// Working directory file actions resolve relative paths against.
    pub workdir: &'a Path,
}

/// The Deploy task.
#[derive(Debug, Default, Clone)]
pub struct DeployTask;

impl DeployTask {
    /// Parses an order from server JSON.
    ///
    /// # Errors
    ///
    /// Returns [`glpi_core::error::AgentError::Json`] if the payload is malformed.
    pub fn parse_order(json: &str) -> Result<DeployOrder> {
        Ok(serde_json::from_str(json)?)
    }

    /// Runs `order` against `ctx`.
    ///
    /// Checks are evaluated first; if a `ko` precondition fails the order is
    /// abandoned. Otherwise every associated file is fetched and verified into
    /// the working directory, then each action runs in turn, stopping at the
    /// first failure.
    ///
    /// # Errors
    ///
    /// Returns an error only for an unrecoverable fault; ordinary check/action
    /// failures are reported in the returned [`DeployReport`].
    pub async fn run(order: &DeployOrder, ctx: &DeployContext<'_>) -> Result<DeployReport> {
        let check_report = CheckProcessor::evaluate(&order.checks, ctx.env);
        if !check_report.proceed {
            return Ok(DeployReport {
                uuid: order.uuid.clone(),
                success: false,
                check_report,
                action_results: Vec::new(),
                message: Some("preconditions not met".to_owned()),
                run_software_inventory: false,
            });
        }

        // Fetch and verify every associated file into the working directory.
        for (sha512, file) in &order.associated_files {
            let target = ctx.workdir.join(&file.name);
            if let Err(err) = assemble(file, ctx.fetcher, &target, Some(sha512)).await {
                return Ok(DeployReport {
                    uuid: order.uuid.clone(),
                    success: false,
                    check_report,
                    action_results: Vec::new(),
                    message: Some(format!("download failed: {err}")),
                    run_software_inventory: false,
                });
            }
        }

        // Run the actions, stopping at the first failure.
        let mut action_results = Vec::with_capacity(order.actions.len());
        let mut success = true;
        let mut message = None;
        for action in &order.actions {
            let result = run_one_action(action, ctx)?;
            let failed = !result.success;
            if failed {
                message = result.message.clone();
            }
            action_results.push(result);
            if failed {
                success = false;
                break;
            }
        }

        Ok(DeployReport {
            uuid: order.uuid.clone(),
            success,
            check_report,
            action_results,
            message,
            run_software_inventory: success && order.run_software_inventory,
        })
    }
}

/// Runs a single action, resolving file paths against the working directory.
fn run_one_action(action: &DeployAction, ctx: &DeployContext<'_>) -> Result<ActionResult> {
    match action {
        DeployAction::Cmd(cmd) => {
            let outcome = run_action(cmd, ctx.runner)?;
            Ok(ActionResult {
                success: outcome.success,
                message: outcome.message,
            })
        }
        DeployAction::Move { from, to } => Ok(fs_result(std::fs::rename(
            resolve(ctx.workdir, from),
            resolve(ctx.workdir, to),
        ))),
        DeployAction::Mkdir { path } => Ok(fs_result(std::fs::create_dir_all(resolve(
            ctx.workdir,
            path,
        )))),
        DeployAction::Delete { path } => {
            let target = resolve(ctx.workdir, path);
            let result = if target.is_dir() {
                std::fs::remove_dir_all(&target)
            } else {
                std::fs::remove_file(&target)
            };
            Ok(fs_result(result))
        }
    }
}

/// Resolves `path` against `workdir` when relative, leaving absolute paths as-is.
fn resolve(workdir: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        workdir.join(p)
    }
}

/// Converts an `io::Result` into an [`ActionResult`].
fn fs_result(result: std::io::Result<()>) -> ActionResult {
    match result {
        Ok(()) => ActionResult {
            success: true,
            message: None,
        },
        Err(err) => ActionResult {
            success: false,
            message: Some(err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{DeployContext, DeployTask};
    use crate::checks::MockEnv;
    use crate::checksum::sha512_hex;
    use crate::downloader::MockPartFetcher;
    use crate::executor::{CommandOutput, MockCommandRunner};

    fn ok_runner() -> MockCommandRunner {
        MockCommandRunner {
            output: CommandOutput {
                code: Some(0),
                stdout: "Success".to_owned(),
                stderr: String::new(),
            },
        }
    }

    #[tokio::test]
    async fn full_order_downloads_then_runs_actions() {
        let payload = b"installer-bytes";
        let sha = sha512_hex(payload);
        let order_json = format!(
            r#"{{
                "uuid":"order-1",
                "associatedFiles":{{ "{sha}": {{ "name":"setup.bin","multiparts":["{sha}"] }} }},
                "checks":[{{"type":"directoryExists","path":"/opt"}}],
                "actions":[
                    {{"mkdir":{{"path":"target"}}}},
                    {{"cmd":{{"exec":"./setup.bin","retChecks":[{{"type":"okPattern","values":["Success"]}}]}}}}
                ],
                "runSoftwareInventory":true
            }}"#
        );
        let order = DeployTask::parse_order(&order_json).unwrap();

        let env = MockEnv::new().with_dir("/opt");
        let runner = ok_runner();
        let fetcher = MockPartFetcher::new().with_part(payload);
        let workdir = tempfile::tempdir().unwrap();
        let ctx = DeployContext {
            env: &env,
            runner: &runner,
            fetcher: &fetcher,
            workdir: workdir.path(),
        };

        let report = DeployTask::run(&order, &ctx).await.unwrap();
        assert!(report.success, "message: {:?}", report.message);
        assert_eq!(report.action_results.len(), 2);
        assert!(report.run_software_inventory);
        // The downloaded file and the mkdir action both landed in the workdir.
        assert!(workdir.path().join("setup.bin").exists());
        assert!(workdir.path().join("target").is_dir());
    }

    #[tokio::test]
    async fn failed_precondition_skips_everything() {
        let order = DeployTask::parse_order(
            r#"{"uuid":"o","checks":[{"type":"fileExists","path":"/missing"}],"actions":[{"mkdir":{"path":"x"}}]}"#,
        )
        .unwrap();
        let env = MockEnv::new();
        let runner = ok_runner();
        let fetcher = MockPartFetcher::new();
        let workdir = tempfile::tempdir().unwrap();
        let ctx = DeployContext {
            env: &env,
            runner: &runner,
            fetcher: &fetcher,
            workdir: workdir.path(),
        };

        let report = DeployTask::run(&order, &ctx).await.unwrap();
        assert!(!report.success);
        assert!(report.action_results.is_empty());
        assert!(!report.run_software_inventory);
        assert!(!workdir.path().join("x").exists());
    }

    #[tokio::test]
    async fn action_failure_stops_the_run() {
        let order = DeployTask::parse_order(
            r#"{"uuid":"o","actions":[
                {"cmd":{"exec":"bad","retChecks":[{"type":"okCode","values":[0]}]}},
                {"mkdir":{"path":"never"}}
            ]}"#,
        )
        .unwrap();
        let env = MockEnv::new();
        let runner = MockCommandRunner {
            output: CommandOutput {
                code: Some(1),
                stdout: String::new(),
                stderr: String::new(),
            },
        };
        let fetcher = MockPartFetcher::new();
        let workdir = tempfile::tempdir().unwrap();
        let ctx = DeployContext {
            env: &env,
            runner: &runner,
            fetcher: &fetcher,
            workdir: workdir.path(),
        };

        let report = DeployTask::run(&order, &ctx).await.unwrap();
        assert!(!report.success);
        // Only the first (failing) action ran; the second was skipped.
        assert_eq!(report.action_results.len(), 1);
        assert!(!workdir.path().join("never").exists());
    }
}
