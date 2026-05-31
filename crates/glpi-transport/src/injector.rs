// SPDX-License-Identifier: GPL-2.0-only

//! The inventory injector.
//!
//! [`Injector`] replays inventory files that were produced earlier (by a
//! `--local` run, or captured from another agent) to a GLPI server. This is the
//! Rust counterpart of the upstream `glpi-injector` tool: it reads a file,
//! decides whether it is JSON or XML, and forwards the bytes verbatim with the
//! matching `Content-Type` via [`GlpiClient::submit_raw`].
//!
//! Files are sent as-is — the injector deliberately does not parse or rewrite
//! them, so a server receives exactly what the producing agent generated.

use std::path::Path;

use glpi_core::error::{AgentError, Result};

use crate::client::GlpiClient;

/// The wire format of an inventory file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFormat {
    /// GLPI native JSON (`.json`).
    Json,
    /// FusionInventory XML (`.xml`, `.ocs`).
    Xml,
}

impl ContentFormat {
    /// The HTTP `Content-Type` for this format.
    #[must_use]
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Xml => "application/xml",
        }
    }

    /// Infers the format from a file extension (case-insensitive).
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Unsupported`] for an unknown or missing extension.
    pub fn from_path(path: &Path) -> Result<Self> {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("json") => Ok(Self::Json),
            Some("xml" | "ocs") => Ok(Self::Xml),
            other => Err(AgentError::Unsupported(format!(
                "cannot infer inventory format from extension {other:?}"
            ))),
        }
    }
}

/// Replays inventory files to a GLPI server through a [`GlpiClient`].
#[derive(Debug, Clone)]
pub struct Injector {
    client: GlpiClient,
}

impl Injector {
    /// Wraps a configured client.
    #[must_use]
    pub fn new(client: GlpiClient) -> Self {
        Self { client }
    }

    /// Reads `path` and submits its contents, inferring the format from the
    /// file extension.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Unsupported`] if the format cannot be inferred,
    /// [`AgentError::Io`] if the file cannot be read, or a transport/auth error
    /// from the submission.
    pub async fn inject_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let format = ContentFormat::from_path(path)?;
        let body = std::fs::read(path)?;
        self.client.submit_raw(body, format.content_type()).await
    }

    /// Submits an in-memory inventory body in the given `format`.
    ///
    /// # Errors
    ///
    /// Returns a transport/auth error from the submission.
    pub async fn inject_bytes(&self, body: Vec<u8>, format: ContentFormat) -> Result<()> {
        self.client.submit_raw(body, format.content_type()).await
    }
}

#[cfg(test)]
mod tests {
    use super::ContentFormat;
    use std::path::Path;

    #[test]
    fn infers_json() {
        assert_eq!(
            ContentFormat::from_path(Path::new("host.json")).unwrap(),
            ContentFormat::Json
        );
    }

    #[test]
    fn infers_xml_variants() {
        assert_eq!(
            ContentFormat::from_path(Path::new("host.xml")).unwrap(),
            ContentFormat::Xml
        );
        assert_eq!(
            ContentFormat::from_path(Path::new("host.OCS")).unwrap(),
            ContentFormat::Xml
        );
    }

    #[test]
    fn rejects_unknown_extension() {
        assert!(ContentFormat::from_path(Path::new("host.txt")).is_err());
        assert!(ContentFormat::from_path(Path::new("host")).is_err());
    }

    #[test]
    fn content_types() {
        assert_eq!(ContentFormat::Json.content_type(), "application/json");
        assert_eq!(ContentFormat::Xml.content_type(), "application/xml");
    }
}
