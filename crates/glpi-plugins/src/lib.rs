// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-plugins` — embedded HTTP-server plugins.
//!
//! Ported from the upstream `GLPI::Agent::HTTP::Server::*` plugins:
//!
//! - [`proxy`] — the **Proxy** plugin (v3.0): accept inventory submissions from
//!   other agents and store and/or forward them to the configured GLPI servers,
//!   with a pass-through depth guard against proxy loops.
//! - [`ssl`] — the **SSL** plugin (v2.0): serve the agent's HTTP server over
//!   HTTPS on a dedicated port from a configured certificate / key / cipher.
//!
//! Each plugin implements [`Plugin`] (identity + listener config). This crate
//! owns their configuration parsing and decision logic; the request routing and
//! the TLS listener are wired into the embedded server (`glpi-http`).

pub mod plugin;
pub mod proxy;
pub mod ssl;

pub use plugin::Plugin;
pub use proxy::{ProxyConfig, ProxyPlan, PASS_THROUGH_HEADER};
pub use ssl::SslConfig;
