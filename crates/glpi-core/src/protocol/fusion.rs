// SPDX-License-Identifier: GPL-2.0-only

//! FusionInventory XML compatibility protocol.
//!
//! Older GLPI servers (and the FusionInventory plugin) speak XML rather than
//! the native JSON of [`super::glpi`]. The agent wraps every message in a
//! `<REQUEST>` element carrying a `<DEVICEID>`, a `<QUERY>` discriminator and,
//! for submissions, a `<CONTENT>` block:
//!
//! ```xml
//! <?xml version="1.0" encoding="UTF-8"?>
//! <REQUEST>
//!   <DEVICEID>agent-123</DEVICEID>
//!   <QUERY>INVENTORY</QUERY>
//!   <CONTENT>…</CONTENT>
//! </REQUEST>
//! ```
//!
//! [`Request`] is generic over its `CONTENT` so the typed per-task payload is
//! plugged in by the task crates, exactly like [`super::glpi::InventoryRequest`].

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{AgentError, Result};

/// XML declaration prepended to every serialized request.
const XML_DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

/// The kind of FusionInventory request, serialized as the `<QUERY>` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Query {
    /// Handshake requesting the task plan.
    Prolog,
    /// A local inventory submission.
    Inventory,
    /// A network-discovery result submission.
    NetDiscovery,
    /// A network-inventory result submission.
    NetInventory,
}

/// A FusionInventory `<REQUEST>` envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "REQUEST")]
pub struct Request<C> {
    /// Stable identifier of this agent (`<DEVICEID>`).
    #[serde(rename = "DEVICEID")]
    pub deviceid: String,
    /// The request discriminator (`<QUERY>`).
    #[serde(rename = "QUERY")]
    pub query: Query,
    /// The payload (`<CONTENT>`); absent for a [`Query::Prolog`] handshake.
    #[serde(
        rename = "CONTENT",
        skip_serializing_if = "Option::is_none",
        default = "no_content"
    )]
    pub content: Option<C>,
}

/// `Option::None` for any `C`, used as the `CONTENT` field default so that
/// deserialization does not require `C: Default` (a bare `#[serde(default)]`
/// would).
fn no_content<C>() -> Option<C> {
    None
}

impl<C> Request<C> {
    /// Builds a `PROLOG` handshake request (no content).
    #[must_use]
    pub fn prolog(deviceid: impl Into<String>) -> Self {
        Self {
            deviceid: deviceid.into(),
            query: Query::Prolog,
            content: None,
        }
    }

    /// Builds an `INVENTORY` submission carrying `content`.
    pub fn inventory(deviceid: impl Into<String>, content: C) -> Self {
        Self {
            deviceid: deviceid.into(),
            query: Query::Inventory,
            content: Some(content),
        }
    }
}

impl<C: Serialize> Request<C> {
    /// Serializes the request to an XML string, including the XML declaration.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Protocol`] if the content cannot be serialized.
    pub fn to_xml(&self) -> Result<String> {
        let body = quick_xml::se::to_string(self)
            .map_err(|e| AgentError::Protocol(format!("FusionInventory XML serialization: {e}")))?;
        Ok(format!("{XML_DECLARATION}\n{body}"))
    }
}

impl<C: DeserializeOwned> Request<C> {
    /// Parses a request from an XML string.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Protocol`] if the XML is malformed or does not
    /// match the expected `<REQUEST>` shape.
    pub fn from_xml(xml: &str) -> Result<Self> {
        quick_xml::de::from_str(xml)
            .map_err(|e| AgentError::Protocol(format!("FusionInventory XML parsing: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{Query, Request};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Hardware {
        #[serde(rename = "NAME")]
        name: String,
    }

    #[test]
    fn prolog_has_no_content() {
        let xml = Request::<Hardware>::prolog("agent-1").to_xml().unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<DEVICEID>agent-1</DEVICEID>"));
        assert!(xml.contains("<QUERY>PROLOG</QUERY>"));
        assert!(!xml.contains("<CONTENT"));
    }

    #[test]
    fn inventory_wraps_content() {
        let req = Request::inventory(
            "agent-1",
            Hardware {
                name: "host".to_owned(),
            },
        );
        let xml = req.to_xml().unwrap();
        assert!(xml.contains("<QUERY>INVENTORY</QUERY>"));
        assert!(xml.contains("<CONTENT>"));
        assert!(xml.contains("<NAME>host</NAME>"));
    }

    #[test]
    fn round_trips_through_xml() {
        let req = Request::inventory(
            "agent-1",
            Hardware {
                name: "host".to_owned(),
            },
        );
        let xml = req.to_xml().unwrap();
        let parsed = Request::<Hardware>::from_xml(&xml).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn query_serializes_uppercase() {
        assert_eq!(
            serde_json::to_value(Query::NetInventory).unwrap(),
            "NETINVENTORY"
        );
    }
}
