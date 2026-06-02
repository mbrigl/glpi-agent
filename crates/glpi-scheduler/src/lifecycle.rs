// SPDX-License-Identifier: GPL-2.0-only

//! Daemon process lifecycle: a PID file and background detach.
//!
//! Ported from the upstream agent's daemon handling:
//!
//! * [`PidFile`] writes the running process's id to a file and removes it on
//!   drop, refusing to start when another live instance already holds it (a
//!   stale file from a crashed instance is taken over);
//! * [`detach`] puts the daemon in the background. Because a multi-threaded
//!   async runtime makes a bare `fork()` unsafe, this re-execs the agent as a
//!   detached child (`setsid`, null stdio) carrying a [`DETACH_ENV`] marker, and
//!   the foreground process exits.

use std::fs;
use std::path::{Path, PathBuf};

use glpi_core::error::{AgentError, Result};

/// Environment marker set on the re-exec'd child so it does not detach again.
pub const DETACH_ENV: &str = "GLPI_AGENT_DETACHED";

/// A held PID file: written on [`acquire`](PidFile::acquire), removed on drop.
#[derive(Debug)]
pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Acquires `path` for this process.
    ///
    /// # Errors
    ///
    /// [`AgentError::Config`] if another **live** instance already holds the
    /// file, or the file cannot be written. A file whose owner is no longer
    /// running is treated as stale and taken over (Unix only; on other
    /// platforms there is no portable liveness probe, so an existing file is
    /// conservatively treated as live — see `process_is_alive`).
    pub fn acquire(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(pid) = read_pid(&path) {
            if pid != std::process::id() && process_is_alive(pid) {
                return Err(AgentError::Config(format!(
                    "another agent is already running (pid {pid}, {})",
                    path.display()
                )));
            }
        }
        fs::write(&path, format!("{}\n", std::process::id())).map_err(|e| {
            AgentError::Config(format!("cannot write pid file {}: {e}", path.display()))
        })?;
        Ok(Self { path })
    }

    /// The path of the held PID file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        // Only remove the file if it still records our pid, so we never delete a
        // successor instance's pid file.
        if read_pid(&self.path) == Some(std::process::id()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Reads the pid recorded in `path`, if it holds a valid number.
fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Whether process `pid` is currently alive.
#[cfg(unix)]
#[must_use]
#[allow(unsafe_code)] // `libc::kill(pid, 0)` is the portable liveness probe.
fn process_is_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: `kill` with signal 0 performs the permission/existence checks
    // without delivering a signal.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    // EPERM means the process exists but we are not allowed to signal it.
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
#[must_use]
fn process_is_alive(_pid: u32) -> bool {
    // Without a portable probe, assume a recorded pid is live (safer: refuse to
    // start over an existing file rather than risk two instances).
    true
}

/// Whether this process is the re-exec'd, detached child (see [`detach`]).
#[must_use]
pub fn is_detached_child() -> bool {
    std::env::var_os(DETACH_ENV).is_some()
}

/// Detaches the agent into the background by re-execing itself as a session
/// leader with null stdio, then returning so the foreground process can exit.
///
/// The child carries the [`DETACH_ENV`] marker so it runs the daemon directly
/// instead of detaching again.
///
/// # Errors
///
/// [`AgentError::Config`] if the executable path cannot be found or the child
/// cannot be spawned.
#[cfg(unix)]
#[allow(unsafe_code)] // `pre_exec(setsid)` detaches the child's session.
pub fn detach() -> Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let exe = std::env::current_exe()
        .map_err(|e| AgentError::Config(format!("cannot find the agent executable: {e}")))?;
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    let mut command = Command::new(exe);
    command
        .args(&args)
        .env(DETACH_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: `setsid` in the child detaches it from the controlling terminal;
    // it is async-signal-safe and we allocate nothing between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command
        .spawn()
        .map_err(|e| AgentError::Config(format!("cannot spawn the detached agent: {e}")))?;
    Ok(())
}

/// Background detach is Unix-only (it relies on `fork`/`setsid`). On other
/// platforms this is unsupported; run the agent as a managed service instead.
///
/// # Errors
///
/// Always returns [`AgentError::Config`] explaining that detach is Unix-only.
#[cfg(not(unix))]
pub fn detach() -> Result<()> {
    Err(AgentError::Config(
        "background detach is only supported on Unix; run as a service instead".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::PidFile;

    #[test]
    fn acquire_writes_our_pid_and_drop_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.pid");
        {
            let pidfile = PidFile::acquire(&path).unwrap();
            assert_eq!(pidfile.path(), path);
            let recorded: u32 = std::fs::read_to_string(&path)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert_eq!(recorded, std::process::id());
        }
        // Dropped: the file is gone.
        assert!(!path.exists());
    }

    #[test]
    fn reacquiring_in_the_same_process_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.pid");
        let _first = PidFile::acquire(&path).unwrap();
        // Our own pid in the file must not be mistaken for another instance.
        let _second = PidFile::acquire(&path).unwrap();
    }

    // Taking over a stale pid file depends on the liveness probe detecting that
    // the recorded pid is gone, which is Unix-only (see `process_is_alive`); on
    // other platforms acquire conservatively refuses to reuse an existing file.
    #[cfg(unix)]
    #[test]
    fn a_stale_pid_file_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.pid");
        // A pid that is almost certainly not running.
        std::fs::write(&path, "2147483646\n").unwrap();
        let pidfile = PidFile::acquire(&path).unwrap();
        let recorded: u32 = std::fs::read_to_string(pidfile.path())
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(recorded, std::process::id());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_start_over_a_live_instance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.pid");
        // pid 1 (init) is always alive and is not our pid.
        std::fs::write(&path, "1\n").unwrap();
        let err = PidFile::acquire(&path).unwrap_err();
        assert!(err.to_string().contains("already running"));
        // The live instance's file is left intact.
        assert!(path.exists());
    }
}
