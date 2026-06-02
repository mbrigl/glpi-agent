// SPDX-License-Identifier: GPL-2.0-only

//! End-to-end test of the task-fork child worker: spawns the real `glpi-agent`
//! binary as a `__task-worker`, sends it a task event, and asserts the worker
//! reports back over the IPC protocol on real OS pipes.

use glpi_scheduler::{Event, IpcMessage, TaskWorker};
use tokio::process::Command;

/// Spawns the worker for the deterministic `selftest` task and checks the full
/// log → progress → result → done exchange round-trips through a child process.
#[tokio::test]
async fn task_worker_runs_selftest_over_real_ipc() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_glpi-agent"));
    command.arg("__task-worker").env("RUST_LOG", "off");

    let event = Event::run_now("selftest", 0, Default::default());
    let worker = TaskWorker::spawn(command, &event)
        .await
        .expect("spawning the worker");

    let mut logs = 0usize;
    let mut progress = 0usize;
    let outcome = worker
        .run_to_completion(|message| match message {
            IpcMessage::Log { .. } => logs += 1,
            IpcMessage::Progress { .. } => progress += 1,
            _ => {}
        })
        .await
        .expect("running the worker to completion");

    assert!(outcome.success, "worker should report success");
    assert_eq!(
        outcome.results,
        vec![("text/plain".to_owned(), b"ok".to_vec())]
    );
    assert!(logs >= 1, "worker should emit at least one log line");
    assert_eq!(progress, 1, "selftest emits a single progress update");
}

/// An unknown task is reported as a failed `Done` rather than crashing.
#[tokio::test]
async fn task_worker_rejects_an_unknown_task() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_glpi-agent"));
    command.arg("__task-worker").env("RUST_LOG", "off");

    let event = Event::run_now("does-not-exist", 0, Default::default());
    let worker = TaskWorker::spawn(command, &event)
        .await
        .expect("spawning the worker");

    let outcome = worker
        .run_to_completion(|_| {})
        .await
        .expect("running the worker to completion");

    assert!(!outcome.success);
    assert!(outcome
        .message
        .unwrap_or_default()
        .contains("not supported"));
}
