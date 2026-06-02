// SPDX-License-Identifier: GPL-2.0-only

//! The `assetname-support` option for remote targets.
//!
//! Controls how a remote host's asset name (used as the GLPI device id) is
//! derived from its hostname: `1` short, `2` as-is, `3` fully-qualified.

use glpi_core::error::{AgentError, Result};

use crate::session::RemoteSession;

/// How to derive the remote asset name from the hostname.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetnameSupport {
    /// `1` — short hostname (up to the first dot).
    #[default]
    Short,
    /// `2` — the hostname exactly as reported.
    AsIs,
    /// `3` — the fully-qualified domain name.
    Fqdn,
}

impl AssetnameSupport {
    /// Parses the numeric option value (`1`/`2`/`3`).
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Parse`] for any other value.
    pub fn from_option(value: &str) -> Result<Self> {
        match value.trim() {
            "1" => Ok(Self::Short),
            "2" => Ok(Self::AsIs),
            "3" => Ok(Self::Fqdn),
            other => Err(AgentError::Parse(format!(
                "invalid assetname-support {other:?} (expected 1, 2 or 3)"
            ))),
        }
    }

    /// Resolves the asset name over `session` according to this mode.
    ///
    /// # Errors
    ///
    /// Propagates the session failure when the hostname cannot be read.
    pub async fn resolve(self, session: &mut dyn RemoteSession) -> Result<String> {
        let command = match self {
            Self::Fqdn => "hostname -f",
            Self::Short | Self::AsIs => "hostname",
        };
        let hostname = session.run(command).await?;
        Ok(self.apply(hostname.trim()))
    }

    /// Applies the mode to an already-read hostname string (pure / testable).
    #[must_use]
    pub fn apply(self, hostname: &str) -> String {
        match self {
            Self::Short => hostname.split('.').next().unwrap_or(hostname).to_owned(),
            Self::AsIs | Self::Fqdn => hostname.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AssetnameSupport;
    use crate::session::MockSession;

    #[test]
    fn parses_and_applies_modes() {
        assert_eq!(
            AssetnameSupport::from_option("1").unwrap(),
            AssetnameSupport::Short
        );
        assert_eq!(
            AssetnameSupport::from_option("2").unwrap(),
            AssetnameSupport::AsIs
        );
        assert_eq!(
            AssetnameSupport::from_option("3").unwrap(),
            AssetnameSupport::Fqdn
        );
        assert!(AssetnameSupport::from_option("9").is_err());

        assert_eq!(AssetnameSupport::Short.apply("host.corp.example"), "host");
        assert_eq!(
            AssetnameSupport::AsIs.apply("host.corp.example"),
            "host.corp.example"
        );
    }

    #[tokio::test]
    async fn resolves_short_name_over_session() {
        let mut session = MockSession::new().with_command("hostname", "web01.corp.example\n");
        let name = AssetnameSupport::Short.resolve(&mut session).await.unwrap();
        assert_eq!(name, "web01");
    }

    #[tokio::test]
    async fn resolves_fqdn_with_dedicated_command() {
        let mut session = MockSession::new().with_command("hostname -f", "web01.corp.example\n");
        let name = AssetnameSupport::Fqdn.resolve(&mut session).await.unwrap();
        assert_eq!(name, "web01.corp.example");
    }
}
