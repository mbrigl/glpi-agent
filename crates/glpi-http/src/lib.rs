// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-http` — the embedded HTTP control server for the GLPI Agent Rust
//! workspace (v2.0.0).
//!
//! Provides the agent's local control surface:
//!
//! - [`trust`] — [`TrustList`], the `httpd-trust` access control,
//! - [`server`] — [`HttpServer`] serving `/status` and `/now` (axum), gated by
//!   the trust list. A `/now` request is parsed into a typed
//!   [`Event`](glpi_scheduler::Event) and delivered to the daemon.
//! - [`proxy`] — the Proxy server plugin's receive/forward route,
//! - [`tls`] — the HTTPS listener for the SSL server plugin.
//!
//! The ToolBox UI pages (plan §2) are deferred to a later unit.

pub mod proxy;
pub mod server;
pub mod tls;
pub mod trust;

pub use proxy::{InventoryForwarder, TransportForwarder};
pub use server::{HttpServer, DEFAULT_HTTP_PORT};
pub use tls::{serve_tls, server_config};
pub use trust::TrustList;
