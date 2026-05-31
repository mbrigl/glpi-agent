// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-transport` — the HTTP transport that carries GLPI native protocol
//! messages between the agent and a GLPI server.
//!
//! The entry point is [`GlpiClient`]: build it from a server endpoint URL with
//! [`GlpiClient::new`], or via [`GlpiClientBuilder`] to configure Basic auth,
//! TLS trust (custom CA, client certificate, `no-ssl-check`) and timeouts. Then
//! perform the `contact` handshake and submit inventories. The protocol message
//! types themselves live in [`glpi_core::protocol`].

mod client;

pub use client::{GlpiClient, GlpiClientBuilder, DEFAULT_TIMEOUT, DEFAULT_USER_AGENT};
