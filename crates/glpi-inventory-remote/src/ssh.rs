// SPDX-License-Identifier: GPL-2.0-only

//! SSH mode 1: drive the system `ssh` command-line client.
//!
//! [`SshCliSession`] shells out to the OpenSSH `ssh` binary, the most portable
//! transport and the one that needs no extra Rust dependencies (key/agent
//! authentication; for password auth use the russh mode instead). The
//! argument vector is built by the pure [`SshCliSession::ssh_args`] so it can be
//! asserted in tests without contacting a host.

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};

use crate::session::RemoteSession;
use crate::target::RemoteTarget;

/// Default per-connection timeout passed to `ssh -o ConnectTimeout`.
const DEFAULT_CONNECT_TIMEOUT_SECS: u32 = 15;

/// A [`RemoteSession`] backed by the command-line `ssh` client.
#[derive(Debug, Clone)]
pub struct SshCliSession {
    host: String,
    user: Option<String>,
    port: Option<u16>,
    /// `ssh` executable (overridable for tests / non-standard installs).
    ssh_bin: String,
    /// `-i` identity (private key) file.
    identity: Option<String>,
    /// `-o UserKnownHostsFile` — set this explicitly on Windows, where the
    /// default `~/.ssh/known_hosts` path is unreliable (the documented fix).
    known_hosts: Option<String>,
    /// When set, adds `StrictHostKeyChecking=accept-new` for first contact.
    accept_new_host_keys: bool,
    connect_timeout_secs: u32,
}

impl SshCliSession {
    /// Builds an SSH session from a parsed `ssh://` target.
    #[must_use]
    pub fn new(target: &RemoteTarget) -> Self {
        Self {
            host: target.host.clone(),
            user: target.user.clone(),
            port: target.port,
            ssh_bin: "ssh".to_owned(),
            identity: None,
            known_hosts: None,
            accept_new_host_keys: false,
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
        }
    }

    /// Overrides the `ssh` executable path.
    #[must_use]
    pub fn with_ssh_bin(mut self, ssh_bin: impl Into<String>) -> Self {
        self.ssh_bin = ssh_bin.into();
        self
    }

    /// Sets the `-i` identity file.
    #[must_use]
    pub fn with_identity(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    /// Sets an explicit `known_hosts` file (recommended on Windows).
    #[must_use]
    pub fn with_known_hosts(mut self, known_hosts: impl Into<String>) -> Self {
        self.known_hosts = Some(known_hosts.into());
        self
    }

    /// Accepts and records previously-unknown host keys on first contact.
    #[must_use]
    pub fn accept_new_host_keys(mut self, accept: bool) -> Self {
        self.accept_new_host_keys = accept;
        self
    }

    /// The destination `user@host` (or bare `host`).
    fn destination(&self) -> String {
        match &self.user {
            Some(user) => format!("{user}@{}", self.host),
            None => self.host.clone(),
        }
    }

    /// Builds the full `ssh` argument vector for `command` (pure / testable).
    #[must_use]
    pub fn ssh_args(&self, command: &str) -> Vec<String> {
        let mut args = vec![
            "-o".to_owned(),
            "BatchMode=yes".to_owned(),
            "-o".to_owned(),
            format!("ConnectTimeout={}", self.connect_timeout_secs),
        ];
        if self.accept_new_host_keys {
            args.push("-o".to_owned());
            args.push("StrictHostKeyChecking=accept-new".to_owned());
        }
        if let Some(known_hosts) = &self.known_hosts {
            args.push("-o".to_owned());
            args.push(format!("UserKnownHostsFile={known_hosts}"));
        }
        if let Some(identity) = &self.identity {
            args.push("-i".to_owned());
            args.push(identity.clone());
        }
        if let Some(port) = self.port {
            args.push("-p".to_owned());
            args.push(port.to_string());
        }
        args.push(self.destination());
        // Everything after the destination is the remote command; ssh re-joins
        // it and hands it to the login shell, so a single word is enough.
        args.push(command.to_owned());
        args
    }
}

#[async_trait]
impl RemoteSession for SshCliSession {
    async fn run(&mut self, command: &str) -> Result<String> {
        let output = tokio::process::Command::new(&self.ssh_bin)
            .args(self.ssh_args(command))
            .output()
            .await
            .map_err(AgentError::Io)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::Task(format!(
                "ssh to {} failed ({}): {}",
                self.host,
                output.status,
                stderr.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::SshCliSession;
    use crate::target::RemoteTarget;

    fn session(url: &str) -> SshCliSession {
        SshCliSession::new(&RemoteTarget::parse(url).unwrap())
    }

    #[test]
    fn builds_minimal_args() {
        let args = session("ssh://host").ssh_args("uname -r");
        assert_eq!(
            args,
            vec![
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=15",
                "host",
                "uname -r",
            ]
        );
    }

    #[test]
    fn builds_user_port_identity_and_known_hosts() {
        let args = session("ssh://admin@10.0.0.5:2222")
            .with_identity("/keys/id_ed25519")
            .with_known_hosts("C:/agent/known_hosts")
            .accept_new_host_keys(true)
            .ssh_args("hostname");
        assert!(args
            .windows(2)
            .any(|w| w == ["-o", "StrictHostKeyChecking=accept-new"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["-o", "UserKnownHostsFile=C:/agent/known_hosts"]));
        assert!(args.windows(2).any(|w| w == ["-i", "/keys/id_ed25519"]));
        assert!(args.windows(2).any(|w| w == ["-p", "2222"]));
        assert_eq!(args[args.len() - 2], "admin@10.0.0.5");
        assert_eq!(args[args.len() - 1], "hostname");
    }
}
