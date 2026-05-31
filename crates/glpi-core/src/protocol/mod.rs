// SPDX-License-Identifier: GPL-2.0-only

//! Wire protocol between the agent and a GLPI server.
//!
//! Two encodings are in scope for the project:
//!
//! - [`glpi`] — the GLPI *native* JSON protocol (the `contact` handshake and
//!   the `inventory` submission). This is the primary, preferred format.
//! - FusionInventory XML — the legacy compatibility format, added later.
//!
//! [`partial`] holds the category-selection logic shared by both encodings
//! (the `no-category` / `required-category` filters used for partial
//! inventories).

pub mod glpi;
pub mod partial;

pub use glpi::{Action, ContactRequest, ContactResponse, InventoryRequest};
pub use partial::select_categories;
