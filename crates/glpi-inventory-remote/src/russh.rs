// SPDX-License-Identifier: GPL-2.0-only

//! SSH mode 2: a pure-Rust transport via [`russh`] (the libssh2 replacement).
//!
//! [`RusshSession`] opens an SSH connection without depending on a system `ssh`
//! binary, supporting password and private-key authentication. It implements
//! the same [`RemoteSession`] seam as the command-line client, so the inventory
//! orchestrator drives it identically.
//!
//! Host keys are accepted Trust-On-First-Use (matching the upstream agent's
//! default libssh2 policy); persistent `known_hosts` pinning is a follow-up.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};
use russh::client::{self, Config, Handle, Handler};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::ChannelMsg;

use crate::session::RemoteSession;
use crate::target::RemoteTarget;

/// Default TCP/handshake timeout.
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Connection options for [`RusshSession`].
#[derive(Debug, Clone)]
pub struct RusshOptions {
    /// Connection / inactivity timeout.
    pub connect_timeout: Duration,
    /// Private-key files to try for public-key authentication, in order.
    pub identities: Vec<PathBuf>,
}

impl Default for RusshOptions {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            identities: Vec::new(),
        }
    }
}

impl RusshOptions {
    /// Creates default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a private-key file to try for authentication.
    #[must_use]
    pub fn with_identity(mut self, identity: impl Into<PathBuf>) -> Self {
        self.identities.push(identity.into());
        self
    }
}

/// The client handler — accepts the server key Trust-On-First-Use.
struct ClientHandler;

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TOFU: accept the presented key (no persistent pinning yet).
        Ok(true)
    }
}

/// A [`RemoteSession`] backed by an established `russh` connection.
pub struct RusshSession {
    handle: Handle<ClientHandler>,
}

impl RusshSession {
    /// Connects to `target` and authenticates (password, then each identity).
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Task`] on connection failure and
    /// [`AgentError::Auth`] when no authentication method succeeds.
    pub async fn connect(target: &RemoteTarget, options: &RusshOptions) -> Result<Self> {
        let user = resolve_user(target)?;
        let config = Arc::new(Config {
            inactivity_timeout: Some(options.connect_timeout),
            ..Config::default()
        });
        let addr = (target.host.clone(), target.port.unwrap_or(22));

        let mut handle = client::connect(config, addr, ClientHandler)
            .await
            .map_err(|e| AgentError::Task(format!("ssh connect to {} failed: {e}", target.host)))?;

        if Self::authenticate(&mut handle, &user, target, options).await? {
            Ok(Self { handle })
        } else {
            Err(AgentError::Auth(format!(
                "no SSH authentication method succeeded for {user}@{}",
                target.host
            )))
        }
    }

    /// Tries password auth (if a password is set) then each identity key.
    async fn authenticate(
        handle: &mut Handle<ClientHandler>,
        user: &str,
        target: &RemoteTarget,
        options: &RusshOptions,
    ) -> Result<bool> {
        if let Some(password) = &target.password {
            let result = handle
                .authenticate_password(user, password.clone())
                .await
                .map_err(auth_err)?;
            if result.success() {
                return Ok(true);
            }
        }
        for identity in &options.identities {
            let key = load_secret_key(identity, None).map_err(|e| {
                AgentError::Auth(format!("can't load key {}: {e}", identity.display()))
            })?;
            let result = handle
                .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), None))
                .await
                .map_err(auth_err)?;
            if result.success() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[async_trait]
impl RemoteSession for RusshSession {
    async fn run(&mut self, command: &str) -> Result<String> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| AgentError::Task(format!("ssh channel open failed: {e}")))?;
        // Force a stable locale, like the upstream agent.
        channel
            .exec(true, format!("LANG=C {command}").as_bytes())
            .await
            .map_err(|e| AgentError::Task(format!("ssh exec failed: {e}")))?;

        let mut stdout = Vec::new();
        let mut exit_status = None;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status: code } => exit_status = Some(code),
                // stderr (ExtendedData), Eof and Close are not needed here.
                _ => {}
            }
        }

        match exit_status {
            Some(0) | None => Ok(String::from_utf8_lossy(&stdout).into_owned()),
            Some(code) => Err(AgentError::Task(format!(
                "remote command exited with status {code}"
            ))),
        }
    }
}

/// Resolves the login user: the URL user, else `$USER`, else an error.
fn resolve_user(target: &RemoteTarget) -> Result<String> {
    target
        .user
        .clone()
        .or_else(|| std::env::var("USER").ok().filter(|u| !u.is_empty()))
        .ok_or_else(|| AgentError::Auth("no SSH user given and $USER is unset".to_owned()))
}

/// Maps a russh auth-transport error to [`AgentError::Auth`].
fn auth_err(e: russh::Error) -> AgentError {
    AgentError::Auth(format!("ssh authentication error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{resolve_user, RusshOptions};
    use crate::target::RemoteTarget;
    use std::path::PathBuf;

    #[test]
    fn options_default_and_builder() {
        let opts = RusshOptions::new().with_identity("/keys/id_ed25519");
        assert_eq!(opts.identities, vec![PathBuf::from("/keys/id_ed25519")]);
        assert_eq!(opts.connect_timeout, super::DEFAULT_CONNECT_TIMEOUT);
    }

    #[test]
    fn resolve_user_prefers_url_user() {
        let target = RemoteTarget::parse("ssh://alice@host").unwrap();
        assert_eq!(resolve_user(&target).unwrap(), "alice");
    }

    #[test]
    fn resolve_user_errors_without_user_or_env() {
        let target = RemoteTarget::parse("ssh://host").unwrap();
        // Only assert the error path when $USER is absent in the test env.
        if std::env::var("USER").is_err() {
            assert!(resolve_user(&target).is_err());
        }
    }
}
