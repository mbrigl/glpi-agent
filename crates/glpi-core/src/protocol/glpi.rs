// SPDX-License-Identifier: GPL-2.0-only

//! GLPI native JSON protocol messages.
//!
//! The agent talks to `/front/inventory.php` with JSON bodies. Two requests
//! matter here:
//!
//! - [`ContactRequest`] — the `contact` handshake (a.k.a. "prolog"); the server
//!   answers with a [`ContactResponse`] describing which tasks to run.
//! - [`InventoryRequest`] — an `inventory` submission carrying the collected
//!   content for one asset.
//!
//! The request types are intentionally thin: [`InventoryRequest`] is generic
//! over its `content`, so the typed per-category payload (defined later in
//! `glpi-inventory-local`) is plugged in without this crate depending on it.

use serde::{Deserialize, Serialize};

/// The action a request performs, serialized as the lower-case `action` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Handshake that asks the server for the task plan.
    Contact,
    /// Submission of a collected inventory.
    Inventory,
}

/// The `contact` handshake request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContactRequest {
    /// Always [`Action::Contact`].
    pub action: Action,
    /// Stable identifier of this agent (`deviceid`).
    pub deviceid: String,
    /// Inventory tag, if configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Tasks the agent is built with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub installed_tasks: Vec<String>,
    /// Tasks the agent is willing to run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_tasks: Vec<String>,
}

impl ContactRequest {
    /// Builds a `contact` request for the given agent device id.
    #[must_use]
    pub fn new(deviceid: impl Into<String>) -> Self {
        Self {
            action: Action::Contact,
            deviceid: deviceid.into(),
            tag: None,
            installed_tasks: Vec::new(),
            enabled_tasks: Vec::new(),
        }
    }
}

/// The server's reply to a [`ContactRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ContactResponse {
    /// Overall status string (for example `"ok"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// How long (in hours, as sent by the server) the plan stays valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,
    /// Per-task parameters keyed by task name; left opaque at this layer.
    #[serde(default)]
    pub tasks: serde_json::Map<String, serde_json::Value>,
}

/// The default GLPI asset type for a submission when none is specified.
pub const DEFAULT_ITEMTYPE: &str = "Computer";

/// An `inventory` submission carrying collected `content`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryRequest<C> {
    /// Always [`Action::Inventory`].
    pub action: Action,
    /// Stable identifier of this agent (`deviceid`).
    pub deviceid: String,
    /// GLPI asset type (`itemtype`) the content maps to.
    pub itemtype: String,
    /// The collected inventory payload.
    pub content: C,
}

impl<C> InventoryRequest<C> {
    /// Builds an `inventory` submission for the default `Computer` itemtype.
    pub fn new(deviceid: impl Into<String>, content: C) -> Self {
        Self {
            action: Action::Inventory,
            deviceid: deviceid.into(),
            itemtype: DEFAULT_ITEMTYPE.to_owned(),
            content,
        }
    }

    /// Overrides the GLPI `itemtype` (for GLPI 11+ genericity).
    #[must_use]
    pub fn with_itemtype(mut self, itemtype: impl Into<String>) -> Self {
        self.itemtype = itemtype.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, ContactRequest, ContactResponse, InventoryRequest};
    use serde_json::json;

    #[test]
    fn action_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(Action::Contact).unwrap(),
            json!("contact")
        );
        assert_eq!(
            serde_json::to_value(Action::Inventory).unwrap(),
            json!("inventory")
        );
    }

    #[test]
    fn contact_request_shape() {
        let mut req = ContactRequest::new("agent-123");
        req.tag = Some("lab".to_owned());
        req.enabled_tasks = vec!["inventory".to_owned()];
        let value = serde_json::to_value(&req).unwrap();
        assert_eq!(
            value,
            json!({
                "action": "contact",
                "deviceid": "agent-123",
                "tag": "lab",
                "enabled-tasks": ["inventory"],
            })
        );
    }

    #[test]
    fn contact_request_omits_empty_optionals() {
        let value = serde_json::to_value(ContactRequest::new("a")).unwrap();
        assert_eq!(value, json!({ "action": "contact", "deviceid": "a" }));
    }

    #[test]
    fn contact_response_parses_tasks() {
        let resp: ContactResponse = serde_json::from_value(json!({
            "status": "ok",
            "expiration": "24",
            "tasks": { "inventory": { "params": [] } },
        }))
        .unwrap();
        assert_eq!(resp.status.as_deref(), Some("ok"));
        assert!(resp.tasks.contains_key("inventory"));
    }

    #[test]
    fn inventory_request_wraps_content() {
        let req = InventoryRequest::new("agent-123", json!({ "hardware": { "name": "host" } }))
            .with_itemtype("NetworkEquipment");
        let value = serde_json::to_value(&req).unwrap();
        assert_eq!(
            value,
            json!({
                "action": "inventory",
                "deviceid": "agent-123",
                "itemtype": "NetworkEquipment",
                "content": { "hardware": { "name": "host" } },
            })
        );
    }
}
