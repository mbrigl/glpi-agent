// SPDX-License-Identifier: GPL-2.0-only

//! The GLPI HTTP client.

use glpi_core::error::{AgentError, Result};
use glpi_core::protocol::{ContactRequest, ContactResponse, InventoryRequest};
use reqwest::{Client, StatusCode, Url};
use serde::Serialize;

/// The `User-Agent` header sent with every request.
pub const DEFAULT_USER_AGENT: &str = concat!("glpi-agent/", env!("CARGO_PKG_VERSION"));

/// An HTTP client for one GLPI server endpoint.
///
/// The endpoint URL is the full inventory URL (for example
/// `https://glpi.example/front/inventory.php`); every request is `POST`ed to
/// it as JSON. Construct once and reuse: the underlying [`reqwest::Client`]
/// pools connections.
#[derive(Debug, Clone)]
pub struct GlpiClient {
    http: Client,
    endpoint: Url,
    credentials: Option<(String, String)>,
}

impl GlpiClient {
    /// Builds a client for the given endpoint URL.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if `endpoint` is not a valid URL, or
    /// [`AgentError::Transport`] if the HTTP backend cannot be initialized.
    pub fn new(endpoint: &str) -> Result<Self> {
        let endpoint = Url::parse(endpoint)
            .map_err(|e| AgentError::Config(format!("invalid server URL `{endpoint}`: {e}")))?;
        let http = Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .gzip(true)
            .build()
            .map_err(|e| AgentError::Transport(e.to_string()))?;
        Ok(Self {
            http,
            endpoint,
            credentials: None,
        })
    }

    /// Attaches HTTP Basic credentials, sent with every request.
    #[must_use]
    pub fn with_basic_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.credentials = Some((username.into(), password.into()));
        self
    }

    /// Performs the `contact` handshake and returns the server's task plan.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the server responds with a
    /// non-success status, or the body is not a valid [`ContactResponse`].
    pub async fn contact(&self, request: &ContactRequest) -> Result<ContactResponse> {
        let response = self.post(request).await?;
        let response = check_status(response)?;
        response
            .json::<ContactResponse>()
            .await
            .map_err(|e| AgentError::Protocol(format!("invalid contact response: {e}")))
    }

    /// Submits a collected inventory.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the server responds with a
    /// non-success status.
    pub async fn submit_inventory<C: Serialize>(
        &self,
        request: &InventoryRequest<C>,
    ) -> Result<()> {
        let response = self.post(request).await?;
        check_status(response)?;
        Ok(())
    }

    /// Sends a JSON `POST` of `body` to the endpoint.
    async fn post<T: Serialize>(&self, body: &T) -> Result<reqwest::Response> {
        let mut builder = self.http.post(self.endpoint.clone()).json(body);
        if let Some((user, password)) = &self.credentials {
            builder = builder.basic_auth(user, Some(password));
        }
        builder
            .send()
            .await
            .map_err(|e| AgentError::Transport(e.to_string()))
    }
}

/// Maps a non-success HTTP status onto an [`AgentError`], passing successes
/// through unchanged.
fn check_status(response: reqwest::Response) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = format!("server returned HTTP {status}");
    Err(match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => AgentError::Auth(message),
        _ => AgentError::Transport(message),
    })
}

#[cfg(test)]
mod tests {
    use super::GlpiClient;

    #[test]
    fn rejects_invalid_url() {
        let err = GlpiClient::new("not a url").unwrap_err();
        assert!(matches!(err, glpi_core::error::AgentError::Config(_)));
    }

    #[test]
    fn accepts_valid_url() {
        assert!(GlpiClient::new("https://glpi.example/front/inventory.php").is_ok());
    }
}
