// SPDX-License-Identifier: GPL-2.0-only

//! Command execution for Deploy `cmd` actions.
//!
//! An action runs a command and is judged by its *return checks*: an exit code
//! that must be in an allow-list, output that must contain an expected pattern,
//! and/or output that must not contain an error pattern. Execution goes through
//! the [`CommandRunner`] seam so the evaluation logic is tested without spawning
//! processes; the live [`SystemCommandRunner`] runs through the platform shell
//! (PowerShell on Windows, `sh` elsewhere).

use serde::Deserialize;

use glpi_core::error::{AgentError, Result};

/// A captured command result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Process exit code (`None` if terminated by a signal).
    pub code: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
}

impl CommandOutput {
    /// stdout and stderr concatenated, for pattern matching.
    fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// A `cmd` action: the command to run and the checks judging its result.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CommandAction {
    /// The command line to execute.
    pub exec: String,
    /// Return checks; an empty list means "exit code 0 is success".
    #[serde(default, rename = "retChecks")]
    pub ret_checks: Vec<RetCheck>,
}

/// A single return check.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RetCheck {
    /// The exit code must be one of these.
    OkCode {
        /// Allowed exit codes.
        values: Vec<i32>,
    },
    /// The output must contain at least one of these substrings.
    OkPattern {
        /// Expected substrings.
        values: Vec<String>,
    },
    /// The output must contain none of these substrings.
    ErrorPattern {
        /// Forbidden substrings.
        values: Vec<String>,
    },
}

/// The outcome of running an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    /// Whether every return check passed.
    pub success: bool,
    /// The command output.
    pub output: CommandOutput,
    /// Reason for failure, when unsuccessful.
    pub message: Option<String>,
}

/// Runs a command and captures its output.
pub trait CommandRunner {
    /// Runs `command` and returns its [`CommandOutput`].
    ///
    /// # Errors
    ///
    /// Returns an error only if the command cannot be spawned at all.
    fn run(&self, command: &str) -> Result<CommandOutput>;
}

/// A [`CommandRunner`] that executes through the platform shell.
#[derive(Debug, Default, Clone)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, command: &str) -> Result<CommandOutput> {
        let output = if cfg!(windows) {
            std::process::Command::new("powershell")
                .args(["-NonInteractive", "-Command", command])
                .output()
        } else {
            std::process::Command::new("sh")
                .args(["-c", command])
                .output()
        }
        .map_err(|e| AgentError::Task(format!("spawning {command:?}: {e}")))?;
        Ok(CommandOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Runs `action` through `runner` and evaluates its return checks.
///
/// # Errors
///
/// Propagates a spawn failure from the runner.
pub fn run_action(action: &CommandAction, runner: &dyn CommandRunner) -> Result<ActionOutcome> {
    let output = runner.run(&action.exec)?;
    let message = evaluate_checks(&action.ret_checks, &output);
    Ok(ActionOutcome {
        success: message.is_none(),
        output,
        message,
    })
}

/// Returns the first failure message, or `None` if every check passes. With no
/// checks, exit code 0 is the implicit success condition.
fn evaluate_checks(checks: &[RetCheck], output: &CommandOutput) -> Option<String> {
    if checks.is_empty() {
        return if output.code == Some(0) {
            None
        } else {
            Some(format!("exit code {:?} (expected 0)", output.code))
        };
    }
    let combined = output.combined();
    for check in checks {
        match check {
            RetCheck::OkCode { values } => {
                if !output.code.is_some_and(|c| values.contains(&c)) {
                    return Some(format!("exit code {:?} not in {values:?}", output.code));
                }
            }
            RetCheck::OkPattern { values } => {
                if !values.iter().any(|p| combined.contains(p)) {
                    return Some(format!("output missing expected pattern {values:?}"));
                }
            }
            RetCheck::ErrorPattern { values } => {
                if let Some(found) = values.iter().find(|p| combined.contains(p.as_str())) {
                    return Some(format!("output contains error pattern {found:?}"));
                }
            }
        }
    }
    None
}

/// A [`CommandRunner`] with a single canned output, for tests.
#[derive(Debug, Clone)]
pub struct MockCommandRunner {
    /// The output every `run` returns.
    pub output: CommandOutput,
}

impl CommandRunner for MockCommandRunner {
    fn run(&self, _command: &str) -> Result<CommandOutput> {
        Ok(self.output.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{run_action, CommandAction, CommandOutput, MockCommandRunner};

    fn runner(code: i32, stdout: &str, stderr: &str) -> MockCommandRunner {
        MockCommandRunner {
            output: CommandOutput {
                code: Some(code),
                stdout: stdout.to_owned(),
                stderr: stderr.to_owned(),
            },
        }
    }

    #[test]
    fn no_checks_means_exit_zero_is_success() {
        let action = CommandAction {
            exec: "x".to_owned(),
            ret_checks: Vec::new(),
        };
        assert!(run_action(&action, &runner(0, "", "")).unwrap().success);
        assert!(!run_action(&action, &runner(1, "", "")).unwrap().success);
    }

    #[test]
    fn ok_code_allows_nonzero() {
        let action: CommandAction = serde_json::from_str(
            r#"{"exec":"setup.exe","retChecks":[{"type":"okCode","values":[0,3010]}]}"#,
        )
        .unwrap();
        assert!(run_action(&action, &runner(3010, "", "")).unwrap().success);
        assert!(!run_action(&action, &runner(1, "", "")).unwrap().success);
    }

    #[test]
    fn ok_and_error_patterns_match_output() {
        let action: CommandAction = serde_json::from_str(
            r#"{"exec":"install","retChecks":[
                {"type":"okPattern","values":["Success"]},
                {"type":"errorPattern","values":["FATAL"]}
            ]}"#,
        )
        .unwrap();
        assert!(
            run_action(&action, &runner(0, "Install Success", ""))
                .unwrap()
                .success
        );

        // okPattern present *and* an error pattern -> the error pattern wins.
        let fail = run_action(&action, &runner(0, "Success", "FATAL: disk full")).unwrap();
        assert!(!fail.success);
        assert!(fail.message.unwrap().contains("error pattern"));

        let missing = run_action(&action, &runner(0, "nothing useful", "")).unwrap();
        assert!(!missing.success);
        assert!(missing.message.unwrap().contains("expected pattern"));
    }
}
