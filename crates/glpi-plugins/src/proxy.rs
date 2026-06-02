// SPDX-License-Identifier: GPL-2.0-only

//! Proxy server plugin (v3.0).
//!
//! Ported from `GLPI::Agent::HTTP::Server::Proxy`: an HTTP endpoint that other
//! agents submit inventories to, which this agent stores locally and/or
//! forwards to its configured GLPI servers — a relay for agents that cannot
//! reach the server directly.
//!
//! This models the plugin's configuration and its forwarding decision (local
//! store vs. forward, the pass-through depth guard against proxy loops). The
//! HTTP receive path and the actual forward over `glpi-transport` are wired by
//! the server.

use std::collections::BTreeMap;

use crate::plugin::{bool_or, parse_or, string_opt, Plugin};

/// HTTP header carrying the proxy pass-through depth (incremented per hop).
pub const PASS_THROUGH_HEADER: &str = "GLPI-Proxy-ID";

/// Proxy plugin configuration (`proxy-server-plugin.cfg`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    /// Whether the plugin is disabled.
    pub disabled: bool,
    /// URL path the plugin serves (default `/proxy`).
    pub url_path: String,
    /// Extra listener port (`0` = main HTTP port).
    pub port: u16,
    /// Store submissions locally rather than (or as well as) forwarding.
    pub only_local_store: bool,
    /// Directory to store received inventories in (empty = none).
    pub local_store: Option<String>,
    /// `prolog_freq` advertised to submitting agents (hours).
    pub prolog_freq: u32,
    /// Maximum proxy hops before a submission is refused (loop guard).
    pub max_pass_through: u32,
    /// Whether the GLPI native protocol is offered.
    pub glpi_protocol: bool,
    /// Refuse requests from non-trusted clients.
    pub forbid_not_trusted: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            disabled: true,
            url_path: "/proxy".to_owned(),
            port: 0,
            only_local_store: false,
            local_store: None,
            prolog_freq: 24,
            max_pass_through: 5,
            glpi_protocol: true,
            forbid_not_trusted: false,
        }
    }
}

impl ProxyConfig {
    /// Parses the plugin config map (the `*.cfg` key/values).
    #[must_use]
    pub fn from_config(config: &BTreeMap<String, String>) -> Self {
        let default = Self::default();
        Self {
            disabled: bool_or(config, "disabled", default.disabled),
            url_path: string_opt(config, "url_path").unwrap_or(default.url_path),
            port: parse_or(config, "port", default.port),
            only_local_store: bool_or(config, "only_local_store", default.only_local_store),
            local_store: string_opt(config, "local_store"),
            prolog_freq: parse_or(config, "prolog_freq", default.prolog_freq),
            max_pass_through: parse_or(config, "max_pass_through", default.max_pass_through),
            glpi_protocol: bool_or(config, "glpi_protocol", default.glpi_protocol),
            forbid_not_trusted: bool_or(config, "forbid_not_trusted", default.forbid_not_trusted),
        }
    }

    /// Resolves whether submissions are only stored locally: the configured
    /// flag, or forced on when the GLPI protocol is offered but no GLPI server
    /// is configured (nothing to forward to).
    #[must_use]
    pub fn effective_only_local_store(&self, glpi_server_count: usize) -> bool {
        self.only_local_store || (self.glpi_protocol && glpi_server_count == 0)
    }

    /// Plans how to handle a received submission given the configured GLPI
    /// `servers`, the request's current pass-through `depth`, and whether the
    /// client is trusted.
    #[must_use]
    pub fn plan(&self, servers: &[String], depth: u32, trusted: bool) -> ProxyPlan {
        if self.forbid_not_trusted && !trusted {
            return ProxyPlan::Reject {
                code: 403,
                status: "UNTRUSTED-CLIENT",
            };
        }
        // Refuse once the submission has already passed through too many proxies.
        if depth >= self.max_pass_through {
            return ProxyPlan::Reject {
                code: 403,
                status: "LIMITED-PROXY",
            };
        }
        let store_locally = self.local_store.is_some();
        let forward_to = if self.effective_only_local_store(servers.len()) {
            Vec::new()
        } else {
            servers.to_vec()
        };
        ProxyPlan::Accept {
            store_locally,
            forward_to,
            // The hop count this agent stamps on an onward forward.
            next_depth: depth + 1,
        }
    }
}

impl Plugin for ProxyConfig {
    fn name(&self) -> &'static str {
        "proxy"
    }
    fn config_file(&self) -> &'static str {
        "proxy-server-plugin.cfg"
    }
    fn is_disabled(&self) -> bool {
        self.disabled
    }
    fn port(&self) -> u16 {
        self.port
    }
}

/// The decision for a received proxy submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyPlan {
    /// Accept it.
    Accept {
        /// Whether to persist it to `local_store`.
        store_locally: bool,
        /// GLPI servers to forward it to (empty = store-only).
        forward_to: Vec<String>,
        /// Pass-through depth to stamp on an onward forward.
        next_depth: u32,
    },
    /// Refuse it with an HTTP `code` and a short `status`.
    Reject {
        /// HTTP status code.
        code: u16,
        /// Short status label.
        status: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::{ProxyConfig, ProxyPlan};
    use crate::plugin::Plugin;
    use std::collections::BTreeMap;

    fn config(pairs: &[(&str, &str)]) -> ProxyConfig {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        ProxyConfig::from_config(&map)
    }

    #[test]
    fn defaults_and_identity() {
        let proxy = ProxyConfig::default();
        assert!(proxy.is_disabled());
        assert_eq!(proxy.url_path, "/proxy");
        assert_eq!(proxy.max_pass_through, 5);
        assert_eq!(proxy.config_file(), "proxy-server-plugin.cfg");
    }

    #[test]
    fn parses_config() {
        let proxy = config(&[
            ("disabled", "no"),
            ("port", "8888"),
            ("local_store", "/var/lib/glpi/proxy"),
            ("only_local_store", "yes"),
            ("max_pass_through", "2"),
        ]);
        assert!(!proxy.is_disabled());
        assert_eq!(proxy.port(), 8888);
        assert_eq!(proxy.local_store.as_deref(), Some("/var/lib/glpi/proxy"));
        assert!(proxy.only_local_store);
        assert_eq!(proxy.max_pass_through, 2);
    }

    #[test]
    fn forced_local_store_without_a_server() {
        let proxy = config(&[("glpi_protocol", "yes")]);
        // GLPI protocol but no server -> store-only.
        assert!(proxy.effective_only_local_store(0));
        assert!(!proxy.effective_only_local_store(1));
    }

    #[test]
    fn plan_forwards_to_servers() {
        let proxy = config(&[("local_store", "/srv/store")]);
        let servers = vec!["https://glpi/front/inventory.php".to_owned()];
        match proxy.plan(&servers, 0, true) {
            ProxyPlan::Accept {
                store_locally,
                forward_to,
                next_depth,
            } => {
                assert!(store_locally);
                assert_eq!(forward_to, servers);
                assert_eq!(next_depth, 1);
            }
            ProxyPlan::Reject { .. } => panic!("expected accept"),
        }
    }

    #[test]
    fn plan_rejects_loop_and_untrusted() {
        let proxy = config(&[("max_pass_through", "3"), ("forbid_not_trusted", "yes")]);
        // Depth at the limit -> loop guard.
        assert_eq!(
            proxy.plan(&[], 3, true),
            ProxyPlan::Reject {
                code: 403,
                status: "LIMITED-PROXY"
            }
        );
        // Untrusted client -> refused.
        assert_eq!(
            proxy.plan(&[], 0, false),
            ProxyPlan::Reject {
                code: 403,
                status: "UNTRUSTED-CLIENT"
            }
        );
    }
}
