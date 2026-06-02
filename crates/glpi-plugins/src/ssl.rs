// SPDX-License-Identifier: GPL-2.0-only

//! SSL server plugin (v2.0).
//!
//! Ported from `GLPI::Agent::HTTP::Server::SSL`: serves the embedded HTTP
//! server over HTTPS on a dedicated port using the configured certificate /
//! key / cipher. This models the plugin configuration and its validation; the
//! TLS listener is set up by the server from a validated [`SslConfig`].

use std::collections::BTreeMap;

use glpi_core::error::{AgentError, Result};

use crate::plugin::{bool_or, parse_or, string_opt, Plugin};

/// SSL plugin configuration (`ssl-server-plugin.cfg`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SslConfig {
    /// Whether the plugin is disabled.
    pub disabled: bool,
    /// HTTPS listener port (`0` = unset → invalid when enabled).
    pub port: u16,
    /// PEM certificate file.
    pub ssl_cert_file: Option<String>,
    /// PEM private-key file.
    pub ssl_key_file: Option<String>,
    /// Optional cipher string.
    pub ssl_cipher: Option<String>,
    /// Refuse requests from non-trusted clients.
    pub forbid_not_trusted: bool,
}

impl Default for SslConfig {
    fn default() -> Self {
        Self {
            disabled: true,
            port: 0,
            ssl_cert_file: None,
            ssl_key_file: None,
            ssl_cipher: None,
            forbid_not_trusted: false,
        }
    }
}

impl SslConfig {
    /// Parses the plugin config map. Upstream names the port option `ports`.
    #[must_use]
    pub fn from_config(config: &BTreeMap<String, String>) -> Self {
        let default = Self::default();
        Self {
            disabled: bool_or(config, "disabled", default.disabled),
            port: parse_or(config, "ports", default.port),
            ssl_cert_file: string_opt(config, "ssl_cert_file"),
            ssl_key_file: string_opt(config, "ssl_key_file"),
            ssl_cipher: string_opt(config, "ssl_cipher"),
            forbid_not_trusted: bool_or(config, "forbid_not_trusted", default.forbid_not_trusted),
        }
    }

    /// `true` if the plugin is enabled (not disabled).
    #[must_use]
    pub fn enabled(&self) -> bool {
        !self.disabled
    }

    /// Validates an enabled plugin: it needs a port, a certificate and a key.
    ///
    /// # Errors
    ///
    /// [`AgentError::Config`] describing the first missing requirement.
    pub fn validate(&self) -> Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        if self.port == 0 {
            return Err(AgentError::Config(
                "SSL server plugin enabled but no `ports` set".to_owned(),
            ));
        }
        if self.ssl_cert_file.is_none() {
            return Err(AgentError::Config(
                "SSL server plugin enabled but no `ssl_cert_file` set".to_owned(),
            ));
        }
        if self.ssl_key_file.is_none() {
            return Err(AgentError::Config(
                "SSL server plugin enabled but no `ssl_key_file` set".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Plugin for SslConfig {
    fn name(&self) -> &'static str {
        "ssl"
    }
    fn config_file(&self) -> &'static str {
        "ssl-server-plugin.cfg"
    }
    fn is_disabled(&self) -> bool {
        self.disabled
    }
    fn port(&self) -> u16 {
        self.port
    }
}

#[cfg(test)]
mod tests {
    use super::SslConfig;
    use crate::plugin::Plugin;
    use std::collections::BTreeMap;

    fn config(pairs: &[(&str, &str)]) -> SslConfig {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        SslConfig::from_config(&map)
    }

    #[test]
    fn disabled_by_default_and_valid() {
        let ssl = SslConfig::default();
        assert!(ssl.is_disabled());
        assert_eq!(ssl.config_file(), "ssl-server-plugin.cfg");
        // A disabled plugin is always valid.
        assert!(ssl.validate().is_ok());
    }

    #[test]
    fn enabled_requires_port_cert_and_key() {
        // Missing everything.
        assert!(config(&[("disabled", "no")]).validate().is_err());
        // Missing cert/key.
        assert!(config(&[("disabled", "no"), ("ports", "5986")])
            .validate()
            .is_err());
        // Complete config validates.
        let ssl = config(&[
            ("disabled", "no"),
            ("ports", "5986"),
            ("ssl_cert_file", "/etc/glpi/cert.pem"),
            ("ssl_key_file", "/etc/glpi/key.pem"),
            ("ssl_cipher", "HIGH:!aNULL"),
        ]);
        assert!(ssl.enabled());
        assert_eq!(ssl.port(), 5986);
        assert_eq!(ssl.ssl_cipher.as_deref(), Some("HIGH:!aNULL"));
        assert!(ssl.validate().is_ok());
    }
}
