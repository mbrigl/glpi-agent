// SPDX-License-Identifier: GPL-2.0-only

//! Wire protocol between the agent and a GLPI server.
//!
//! Two encodings are in scope for the project:
//!
//! - [`glpi`] — the GLPI *native* JSON protocol (the `contact` handshake and
//!   the `inventory` submission). This is the primary, preferred format.
//! - [`fusion`] — the legacy FusionInventory XML compatibility format.
//!
//! [`partial`] holds the category-selection logic shared by both encodings
//! (the `no-category` / `required-category` filters used for partial
//! inventories).

pub mod fusion;
pub mod glpi;
pub mod partial;

pub use fusion::{Query, Request};
pub use glpi::{Action, ContactRequest, ContactResponse, InventoryRequest};
pub use partial::select_categories;
