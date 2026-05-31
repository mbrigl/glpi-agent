// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-transport` — the HTTP transport that carries GLPI native protocol
//! messages between the agent and a GLPI server.
//!
//! The single entry point is [`GlpiClient`]: build it from a server endpoint
//! URL, optionally attach Basic credentials, then perform the `contact`
//! handshake and submit inventories. The protocol message types themselves
//! live in [`glpi_core::protocol`].

mod client;

pub use client::{GlpiClient, DEFAULT_USER_AGENT};
