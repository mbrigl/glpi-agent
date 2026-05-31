// SPDX-License-Identifier: GPL-2.0-only

//! The fully-resolved agent configuration ([`Options`]) and the per-layer
//! override type ([`PartialOptions`]) used to build it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The default embedded HTTP server port (upstream agent default).
pub const DEFAULT_HTTPD_PORT: u16 = 62_354;

/// The default maximum delay before the first inventory run, in seconds.
pub const DEFAULT_DELAYTIME: u32 = 3_600;

/// A complete, resolved set of agent options.
///
/// This is the result of merging every configuration layer (see
/// [`super`] for the precedence rules). Every field has a concrete value; there
/// are no "unset" states left at this point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Options {
    /// GLPI server URLs to report to.
    pub server: Vec<String>,
    /// Directory to write inventories to instead of (or in addition to) a
    /// server.
    pub local: Option<PathBuf>,
    /// Explicit HTTP proxy URL (`None` = use the environment; an empty/`none`
    /// value disables proxying — handled by the transport layer).
    pub proxy: Option<String>,
    /// Inventory tag attached to every report.
    pub tag: Option<String>,
    /// Tasks to run (empty = the built-in default set).
    pub tasks: Vec<String>,
    /// Tasks to disable.
    pub no_task: Vec<String>,
    /// Inventory categories to exclude.
    pub no_category: Vec<String>,
    /// Maximum delay before the first run, in seconds.
    pub delaytime: u32,
    /// Do not run before the stored `nextRunDate`.
    pub lazy: bool,
    /// Configuration reload interval in seconds (`0` = never).
    pub conf_reload_interval: u32,
    /// Disable the embedded HTTP server.
    pub no_httpd: bool,
    /// Interface the HTTP server binds to (`None` = all interfaces).
    pub httpd_ip: Option<String>,
    /// Port the embedded HTTP server listens on.
    pub httpd_port: u16,
    /// IPs / CIDR ranges trusted to trigger runs over HTTP.
    pub httpd_trust: Vec<String>,
    /// Debug verbosity (`0` = off).
    pub debug: u8,
    /// Skip TLS certificate validation.
    pub no_ssl_check: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            server: Vec::new(),
            local: None,
            proxy: None,
            tag: None,
            tasks: Vec::new(),
            no_task: Vec::new(),
            no_category: Vec::new(),
            delaytime: DEFAULT_DELAYTIME,
            lazy: false,
            conf_reload_interval: 0,
            no_httpd: false,
            httpd_ip: None,
            httpd_port: DEFAULT_HTTPD_PORT,
            httpd_trust: Vec::new(),
            debug: 0,
            no_ssl_check: false,
        }
    }
}

impl Options {
    /// Resolves a final configuration by applying each layer in turn on top of
    /// the defaults. Later layers win, matching the documented precedence
    /// (defaults → `agent.cfg` → `conf.d/*.cfg` → environment → CLI).
    #[must_use]
    pub fn resolve(layers: &[PartialOptions]) -> Self {
        let mut options = Self::default();
        for layer in layers {
            layer.apply(&mut options);
        }
        options
    }
}

/// A single configuration layer: every field is optional, and only the fields
/// that are `Some` override lower-precedence layers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct PartialOptions {
    /// Override for [`Options::server`].
    pub server: Option<Vec<String>>,
    /// Override for [`Options::local`].
    pub local: Option<PathBuf>,
    /// Override for [`Options::proxy`].
    pub proxy: Option<String>,
    /// Override for [`Options::tag`].
    pub tag: Option<String>,
    /// Override for [`Options::tasks`].
    pub tasks: Option<Vec<String>>,
    /// Override for [`Options::no_task`].
    pub no_task: Option<Vec<String>>,
    /// Override for [`Options::no_category`].
    pub no_category: Option<Vec<String>>,
    /// Override for [`Options::delaytime`].
    pub delaytime: Option<u32>,
    /// Override for [`Options::lazy`].
    pub lazy: Option<bool>,
    /// Override for [`Options::conf_reload_interval`].
    pub conf_reload_interval: Option<u32>,
    /// Override for [`Options::no_httpd`].
    pub no_httpd: Option<bool>,
    /// Override for [`Options::httpd_ip`].
    pub httpd_ip: Option<String>,
    /// Override for [`Options::httpd_port`].
    pub httpd_port: Option<u16>,
    /// Override for [`Options::httpd_trust`].
    pub httpd_trust: Option<Vec<String>>,
    /// Override for [`Options::debug`].
    pub debug: Option<u8>,
    /// Override for [`Options::no_ssl_check`].
    pub no_ssl_check: Option<bool>,
}

impl PartialOptions {
    /// Applies this layer's set fields onto `base`.
    pub fn apply(&self, base: &mut Options) {
        macro_rules! overlay {
            ($field:ident) => {
                if let Some(value) = &self.$field {
                    base.$field = value.clone();
                }
            };
        }
        macro_rules! overlay_opt {
            ($field:ident) => {
                if self.$field.is_some() {
                    base.$field = self.$field.clone();
                }
            };
        }

        overlay!(server);
        overlay_opt!(local);
        overlay_opt!(proxy);
        overlay_opt!(tag);
        overlay!(tasks);
        overlay!(no_task);
        overlay!(no_category);
        overlay!(delaytime);
        overlay!(lazy);
        overlay!(conf_reload_interval);
        overlay!(no_httpd);
        overlay_opt!(httpd_ip);
        overlay!(httpd_port);
        overlay!(httpd_trust);
        overlay!(debug);
        overlay!(no_ssl_check);
    }
}

#[cfg(test)]
mod tests {
    use super::{Options, PartialOptions, DEFAULT_HTTPD_PORT};

    #[test]
    fn defaults_are_sane() {
        let o = Options::default();
        assert_eq!(o.httpd_port, DEFAULT_HTTPD_PORT);
        assert!(o.server.is_empty());
        assert_eq!(o.debug, 0);
    }

    #[test]
    fn later_layers_win() {
        let agent_cfg = PartialOptions {
            server: Some(vec!["https://a.example".to_owned()]),
            debug: Some(1),
            ..PartialOptions::default()
        };
        let cli = PartialOptions {
            server: Some(vec!["https://b.example".to_owned()]),
            ..PartialOptions::default()
        };
        let resolved = Options::resolve(&[agent_cfg, cli]);
        // CLI overrode the server...
        assert_eq!(resolved.server, vec!["https://b.example".to_owned()]);
        // ...but the lower layer's debug value survived.
        assert_eq!(resolved.debug, 1);
        // ...and untouched fields keep their defaults.
        assert_eq!(resolved.httpd_port, DEFAULT_HTTPD_PORT);
    }

    #[test]
    fn empty_layer_changes_nothing() {
        let resolved = Options::resolve(&[PartialOptions::default()]);
        assert_eq!(resolved, Options::default());
    }
}
