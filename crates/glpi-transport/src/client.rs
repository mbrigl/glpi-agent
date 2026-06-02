// SPDX-License-Identifier: GPL-2.0-only

//! The GLPI HTTP client and its builder.

use std::path::{Path, PathBuf};
use std::time::Duration;

use glpi_core::error::{AgentError, Result};
use glpi_core::protocol::{ContactRequest, ContactResponse, InventoryRequest};
use reqwest::{Certificate, Client, Identity, StatusCode, Url};
use serde::Serialize;

/// The `User-Agent` header sent with every request.
pub const DEFAULT_USER_AGENT: &str = concat!("glpi-agent/", env!("CARGO_PKG_VERSION"));

/// Default per-request timeout when the builder does not override it.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(180);

/// An HTTP client for one GLPI server endpoint.
///
/// The endpoint URL is the full inventory URL (for example
/// `https://glpi.example/front/inventory.php`); every request is `POST`ed to
/// it as JSON. Construct once and reuse: the underlying [`reqwest::Client`]
/// pools connections.
///
/// Use [`GlpiClient::new`] for a plain client, or [`GlpiClient::builder`] to
/// configure Basic auth, TLS trust (a custom CA, a client certificate) and
/// timeouts before building.
#[derive(Debug, Clone)]
pub struct GlpiClient {
    http: Client,
    endpoint: Url,
    credentials: Option<(String, String)>,
    bearer_token: Option<String>,
}

impl GlpiClient {
    /// Builds a client for the given endpoint URL with default settings.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if `endpoint` is not a valid URL, or
    /// [`AgentError::Transport`] if the HTTP backend cannot be initialized.
    pub fn new(endpoint: &str) -> Result<Self> {
        Self::builder(endpoint)?.build()
    }

    /// Starts a [`GlpiClientBuilder`] for `endpoint`.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if `endpoint` is not a valid URL.
    pub fn builder(endpoint: &str) -> Result<GlpiClientBuilder> {
        let endpoint = Url::parse(endpoint)
            .map_err(|e| AgentError::Config(format!("invalid server URL `{endpoint}`: {e}")))?;
        Ok(GlpiClientBuilder::new(endpoint))
    }

    /// Attaches HTTP Basic credentials, sent with every request.
    ///
    /// Equivalent to [`GlpiClientBuilder::basic_auth`]; kept for ergonomic use
    /// on an already-built client.
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
        let response = check_status(response).await?;
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
        check_status(response).await?;
        Ok(())
    }

    /// Submits an already-serialized inventory body with an explicit
    /// `Content-Type`.
    ///
    /// This is the primitive the injector uses to forward inventory files
    /// verbatim, without re-serializing them through a typed value.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the server responds with a
    /// non-success status.
    pub async fn submit_raw(&self, body: Vec<u8>, content_type: &str) -> Result<()> {
        let builder = self
            .http
            .post(self.endpoint.clone())
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body);
        let response = self
            .apply_auth(builder)
            .send()
            .await
            .map_err(|e| AgentError::Transport(e.to_string()))?;
        check_status(response).await?;
        Ok(())
    }

    /// Sends a JSON `POST` of `body` to the endpoint.
    async fn post<T: Serialize>(&self, body: &T) -> Result<reqwest::Response> {
        let builder = self.http.post(self.endpoint.clone()).json(body);
        self.apply_auth(builder)
            .send()
            .await
            .map_err(|e| AgentError::Transport(e.to_string()))
    }

    /// Adds the configured authentication header to a request.
    ///
    /// A bearer token (OAuth2) takes precedence over Basic credentials when
    /// both are set; with neither, the request is sent unauthenticated.
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.bearer_token {
            builder.bearer_auth(token)
        } else if let Some((user, password)) = &self.credentials {
            builder.basic_auth(user, Some(password))
        } else {
            builder
        }
    }
}

/// Builder for [`GlpiClient`], collecting auth and TLS options.
#[derive(Debug, Clone)]
pub struct GlpiClientBuilder {
    endpoint: Url,
    credentials: Option<(String, String)>,
    bearer_token: Option<String>,
    ca_cert_file: Option<PathBuf>,
    client_cert_file: Option<PathBuf>,
    no_ssl_check: bool,
    timeout: Duration,
}

impl GlpiClientBuilder {
    /// Creates a builder for an already-parsed endpoint URL.
    fn new(endpoint: Url) -> Self {
        Self {
            endpoint,
            credentials: None,
            bearer_token: None,
            ca_cert_file: None,
            client_cert_file: None,
            no_ssl_check: false,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Sets HTTP Basic credentials.
    #[must_use]
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.credentials = Some((username.into(), password.into()));
        self
    }

    /// Sets an OAuth2 bearer token, sent as `Authorization: Bearer …`.
    ///
    /// Used by GLPI 11+ which can authenticate inventory submissions with an
    /// OAuth2 access token. When set, it takes precedence over Basic
    /// credentials.
    #[must_use]
    pub fn oauth_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Trusts an additional CA certificate (PEM) for server verification
    /// (`ca-cert-file`).
    #[must_use]
    pub fn ca_cert_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_cert_file = Some(path.into());
        self
    }

    /// Presents a client certificate + key (a single PEM file holding both) for
    /// mutual TLS (`ssl-cert-file`).
    #[must_use]
    pub fn client_cert_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.client_cert_file = Some(path.into());
        self
    }

    /// Disables TLS certificate validation (`no-ssl-check`).
    ///
    /// This removes the protection TLS provides against impersonation; use it
    /// only against a server whose certificate cannot be validated for a known,
    /// accepted reason.
    #[must_use]
    pub fn no_ssl_check(mut self, disable: bool) -> Self {
        self.no_ssl_check = disable;
        self
    }

    /// Overrides the per-request timeout (default [`DEFAULT_TIMEOUT`]).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Builds the [`GlpiClient`].
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if a certificate file cannot be read or
    /// parsed, or [`AgentError::Transport`] if the HTTP backend cannot be
    /// initialized.
    pub fn build(self) -> Result<GlpiClient> {
        let mut builder = Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .gzip(true)
            .timeout(self.timeout);

        if let Some(path) = &self.ca_cert_file {
            builder = builder.add_root_certificate(load_ca_cert(path)?);
        }
        if let Some(path) = &self.client_cert_file {
            builder = builder.identity(load_identity(path)?);
        }
        if self.no_ssl_check {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let http = builder
            .build()
            .map_err(|e| AgentError::Transport(e.to_string()))?;
        Ok(GlpiClient {
            http,
            endpoint: self.endpoint,
            credentials: self.credentials,
            bearer_token: self.bearer_token,
        })
    }
}

/// Reads a PEM CA certificate from `path`.
fn load_ca_cert(path: &Path) -> Result<Certificate> {
    let pem = std::fs::read(path).map_err(|e| {
        AgentError::Config(format!("cannot read CA cert `{}`: {e}", path.display()))
    })?;
    Certificate::from_pem(&pem)
        .map_err(|e| AgentError::Config(format!("invalid CA cert `{}`: {e}", path.display())))
}

/// Reads a PEM client identity (certificate + private key) from `path`.
fn load_identity(path: &Path) -> Result<Identity> {
    let pem = std::fs::read(path).map_err(|e| {
        AgentError::Config(format!("cannot read client cert `{}`: {e}", path.display()))
    })?;
    Identity::from_pem(&pem)
        .map_err(|e| AgentError::Config(format!("invalid client cert `{}`: {e}", path.display())))
}

/// Maps a non-success HTTP status onto an [`AgentError`], passing successes
/// through unchanged.
///
/// On failure the response body is appended to the message when present: GLPI
/// returns the validation reason there (e.g. `{"status":"error","message":"JSON
/// does not validate. …"}`), which the bare status code would otherwise hide.
/// The body is truncated to keep the error readable.
async fn check_status(response: reqwest::Response) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    let body = body.trim();
    let message = if body.is_empty() {
        format!("server returned HTTP {status}")
    } else {
        const MAX_BODY: usize = 512;
        let detail: String = body.chars().take(MAX_BODY).collect();
        let ellipsis = if body.chars().nth(MAX_BODY).is_some() {
            "…"
        } else {
            ""
        };
        format!("server returned HTTP {status}: {detail}{ellipsis}")
    };
    Err(match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => AgentError::Auth(message),
        _ => AgentError::Transport(message),
    })
}

#[cfg(test)]
mod tests {
    use super::GlpiClient;
    use glpi_core::error::AgentError;

    #[test]
    fn rejects_invalid_url() {
        let err = GlpiClient::new("not a url").unwrap_err();
        assert!(matches!(err, AgentError::Config(_)));
    }

    #[test]
    fn accepts_valid_url() {
        assert!(GlpiClient::new("https://glpi.example/front/inventory.php").is_ok());
    }

    #[test]
    fn builder_applies_options() {
        let client = GlpiClient::builder("https://glpi.example/front/inventory.php")
            .unwrap()
            .basic_auth("scout", "secret")
            .no_ssl_check(true)
            .build()
            .unwrap();
        assert!(client.credentials.is_some());
    }

    #[test]
    fn missing_ca_cert_is_config_error() {
        let err = GlpiClient::builder("https://glpi.example/front/inventory.php")
            .unwrap()
            .ca_cert_file("/nonexistent/ca.pem")
            .build()
            .unwrap_err();
        assert!(matches!(err, AgentError::Config(_)));
    }

    #[test]
    fn invalid_client_cert_is_config_error() {
        let err = GlpiClient::builder("https://glpi.example/front/inventory.php")
            .unwrap()
            .client_cert_file("/nonexistent/client.pem")
            .build()
            .unwrap_err();
        assert!(matches!(err, AgentError::Config(_)));
    }
}
