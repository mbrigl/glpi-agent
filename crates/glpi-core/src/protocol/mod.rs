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
//! inventories). [`delta`] adds the per-device checksum state that turns an
//! unchanged inventory into a small partial submission.

pub mod delta;
pub mod fusion;
pub mod glpi;
pub mod partial;

pub use delta::{plan as plan_delta, DeltaPlan, InventoryMode, InventoryState};
pub use fusion::{Query, Request};
pub use glpi::{Action, ContactRequest, ContactResponse, InventoryRequest};
pub use partial::select_categories;
