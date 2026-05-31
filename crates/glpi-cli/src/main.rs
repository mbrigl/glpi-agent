// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-agent` — command-line entry point for the GLPI Agent Rust workspace
//! (v2.0.0).
//!
//! Subcommands (inventory, netdiscovery, netinventory, esx, remoteinventory,
//! inject, wakeup, daemon) are wired up in a later phase. For now this prints
//! the version so the binary builds and runs end to end.

fn main() {
    println!("glpi-agent {}", env!("CARGO_PKG_VERSION"));
}
