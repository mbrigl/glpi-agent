// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-http` — the embedded HTTP control server for the GLPI Agent Rust
//! workspace (v2.0.0).
//!
//! Provides the agent's local control surface:
//!
//! - [`trust`] — [`TrustList`], the `httpd-trust` access control,
//! - [`server`] — [`HttpServer`] serving `/status` and `/now` (axum), gated by
//!   the trust list.
//!
//! The ToolBox UI pages and the proxy / SSL plugins (plan §2) are deferred to a
//! later unit; this is the daemon's core control endpoint surface.

pub mod server;
pub mod trust;

pub use server::{HttpServer, NowRequest, DEFAULT_HTTP_PORT};
pub use trust::TrustList;
