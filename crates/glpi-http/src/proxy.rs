// SPDX-License-Identifier: GPL-2.0-only

//! Proxy route for the embedded HTTP server.
//!
//! Mounts the [`ProxyConfig`](glpi_plugins::proxy::ProxyConfig) plugin's
//! receive path onto the control server: another agent POSTs an inventory to
//! the proxy's `url_path`, and this agent — per the plugin's
//! [`plan`](glpi_plugins::proxy::ProxyConfig::plan) decision — stores it locally
//! and/or forwards it to the configured GLPI servers, refusing the request when
//! the pass-through loop guard trips or an untrusted client is forbidden.
//!
//! The actual onward submission goes through an [`InventoryForwarder`] so the
//! route is testable without a network; [`TransportForwarder`] is the real
//! `glpi-transport`-backed implementation used in production.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use glpi_core::error::{AgentError, Result};
use glpi_plugins::proxy::{ProxyConfig, ProxyPlan, PASS_THROUGH_HEADER};
use glpi_transport::{ContentFormat, GlpiClient, Injector};

use crate::trust::TrustList;

/// Forwards a received inventory body onward to one configured GLPI server.
///
/// The route depends on this trait rather than `glpi-transport` directly so the
/// forwarding decision can be unit-tested with an in-memory recorder.
#[async_trait]
pub trait InventoryForwarder: Send + Sync {
    /// Submits `body` (already in `format`) to `server`, stamping `next_depth`
    /// as the onward pass-through hop count.
    ///
    /// # Errors
    ///
    /// Returns a transport error if the submission fails.
    async fn forward(
        &self,
        server: &str,
        body: Vec<u8>,
        format: ContentFormat,
        next_depth: u32,
    ) -> Result<()>;
}

/// The production forwarder: builds a `glpi-transport` client per server and
/// submits the raw inventory body.
#[derive(Debug, Default, Clone)]
pub struct TransportForwarder;

#[async_trait]
impl InventoryForwarder for TransportForwarder {
    async fn forward(
        &self,
        server: &str,
        body: Vec<u8>,
        format: ContentFormat,
        next_depth: u32,
    ) -> Result<()> {
        // The transport client does not yet expose per-request headers, so the
        // `GLPI-Proxy-ID` hop stamp is not propagated onward; the loop guard
        // still protects this hop. Tracked as a transport follow-up.
        let _ = next_depth;
        let client = GlpiClient::new(server)?;
        Injector::new(client).inject_bytes(body, format).await
    }
}

/// Shared state for the proxy route.
#[derive(Clone)]
pub(crate) struct ProxyState {
    config: Arc<ProxyConfig>,
    servers: Arc<Vec<String>>,
    trust: Arc<TrustList>,
    forwarder: Arc<dyn InventoryForwarder>,
}

impl ProxyState {
    pub(crate) fn new(
        config: ProxyConfig,
        servers: Vec<String>,
        trust: Arc<TrustList>,
        forwarder: Arc<dyn InventoryForwarder>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            servers: Arc::new(servers),
            trust,
            forwarder,
        }
    }

    pub(crate) fn url_path(&self) -> String {
        self.config.url_path.clone()
    }
}

/// Builds the proxy sub-router (mounted on the plugin's `url_path`).
///
/// It is intentionally **not** wrapped by the server's trust middleware: the
/// proxy applies its own trust decision via the plugin's
/// `forbid_not_trusted` flag, since relays routinely accept submissions from
/// agents that are not in `httpd-trust`.
pub(crate) fn router(state: ProxyState) -> Router {
    let path = state.url_path();
    Router::new().route(&path, post(handle)).with_state(state)
}

/// Picks the inventory content format from the request `Content-Type`.
fn content_format(headers: &HeaderMap) -> ContentFormat {
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if ct.contains("xml") {
        ContentFormat::Xml
    } else {
        ContentFormat::Json
    }
}

/// Reads the current pass-through depth from the request headers.
fn pass_through_depth(headers: &HeaderMap) -> u32 {
    headers
        .get(PASS_THROUGH_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Stores a received submission under `dir`, keyed by its device id when one
/// can be read from the body, else by a timestamp.
fn store_submission(dir: &str, body: &[u8], format: ContentFormat) -> Result<()> {
    let ext = match format {
        ContentFormat::Xml => "xml",
        ContentFormat::Json => "json",
    };
    let name = device_id(body).unwrap_or_else(fallback_name);
    let path = Path::new(dir).join(format!("{name}.{ext}"));
    std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(&path, body))
        .map_err(|e| AgentError::Transport(format!("cannot store proxy submission: {e}")))
}

/// A unique-enough filename when the body carries no device id.
fn fallback_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("submission-{nanos}")
}

/// Extracts and sanitizes a device id from a JSON or XML inventory body.
fn device_id(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let raw = json_value(text, "deviceid")
        .or_else(|| xml_element(text, "DEVICEID"))
        .or_else(|| json_value(text, "DEVICEID"))?;
    let sanitized: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

/// Tiny `"key": "value"` scan (sufficient for the inventory device id).
fn json_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let after = &text[text.find(&needle)? + needle.len()..];
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let rest = after.strip_prefix('"')?;
    rest.find('"').map(|end| rest[..end].to_owned())
}

/// Tiny `<TAG>value</TAG>` scan.
fn xml_element(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim().to_owned())
}

/// The proxy submission handler.
async fn handle(
    State(state): State<ProxyState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let depth = pass_through_depth(&headers);
    let trusted = state.trust.allows(addr.ip());
    match state.config.plan(&state.servers, depth, trusted) {
        ProxyPlan::Reject { code, status } => {
            tracing::debug!(client = %addr.ip(), code, status, "proxy refused submission");
            let code = StatusCode::from_u16(code).unwrap_or(StatusCode::FORBIDDEN);
            (code, format!("{status}\n")).into_response()
        }
        ProxyPlan::Accept {
            store_locally,
            forward_to,
            next_depth,
        } => {
            let format = content_format(&headers);
            if store_locally {
                if let Some(dir) = state.config.local_store.as_deref() {
                    if let Err(e) = store_submission(dir, &body, format) {
                        tracing::warn!(error = %e, "proxy local store failed");
                        return (StatusCode::INTERNAL_SERVER_ERROR, "STORE-ERROR\n")
                            .into_response();
                    }
                }
            }
            for server in forward_to.iter() {
                if let Err(e) = state
                    .forwarder
                    .forward(server, body.to_vec(), format, next_depth)
                    .await
                {
                    tracing::warn!(server = %server, error = %e, "proxy forward failed");
                    return (StatusCode::BAD_GATEWAY, "FORWARD-ERROR\n").into_response();
                }
            }
            (StatusCode::OK, "OK\n").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{device_id, router, InventoryForwarder, ProxyState};
    use crate::trust::TrustList;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::{Request, StatusCode};
    use glpi_core::error::Result;
    use glpi_plugins::proxy::{ProxyConfig, PASS_THROUGH_HEADER};
    use glpi_transport::ContentFormat;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    /// Recorded forward calls: `(server, body, next_depth)`.
    type Calls = Arc<Mutex<Vec<(String, Vec<u8>, u32)>>>;

    #[derive(Default, Clone)]
    struct RecordingForwarder {
        calls: Calls,
    }

    #[async_trait]
    impl InventoryForwarder for RecordingForwarder {
        async fn forward(
            &self,
            server: &str,
            body: Vec<u8>,
            _format: ContentFormat,
            next_depth: u32,
        ) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push((server.to_owned(), body, next_depth));
            Ok(())
        }
    }

    fn config(pairs: &[(&str, &str)]) -> ProxyConfig {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        ProxyConfig::from_config(&map)
    }

    fn app(state: ProxyState, client_ip: [u8; 4]) -> axum::Router {
        router(state).layer(MockConnectInfo(std::net::SocketAddr::from((
            client_ip, 5000,
        ))))
    }

    async fn post(app: axum::Router, depth: Option<u32>, body: &str) -> axum::http::Response<Body> {
        let mut req = Request::builder()
            .method("POST")
            .uri("/proxy")
            .header("content-type", "application/json");
        if let Some(d) = depth {
            req = req.header(PASS_THROUGH_HEADER, d.to_string());
        }
        app.oneshot(req.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn forwards_to_configured_server() {
        let forwarder = RecordingForwarder::default();
        let state = ProxyState::new(
            config(&[("disabled", "no")]),
            vec!["https://glpi.example/front/inventory.php".to_owned()],
            Arc::new(TrustList::default()),
            Arc::new(forwarder.clone()),
        );
        let res = post(app(state, [127, 0, 0, 1]), None, r#"{"deviceid":"host-1"}"#).await;
        assert_eq!(res.status(), StatusCode::OK);

        let calls = forwarder.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "https://glpi.example/front/inventory.php");
        // The onward hop is the received depth (0) + 1.
        assert_eq!(calls[0].2, 1);
    }

    #[tokio::test]
    async fn rejects_when_loop_guard_trips() {
        let forwarder = RecordingForwarder::default();
        let state = ProxyState::new(
            config(&[("disabled", "no"), ("max_pass_through", "2")]),
            vec!["https://glpi.example/front/inventory.php".to_owned()],
            Arc::new(TrustList::default()),
            Arc::new(forwarder.clone()),
        );
        let res = post(
            app(state, [127, 0, 0, 1]),
            Some(2),
            r#"{"deviceid":"host-1"}"#,
        )
        .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        assert!(forwarder.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn untrusted_client_is_refused_when_forbidden() {
        let forwarder = RecordingForwarder::default();
        let state = ProxyState::new(
            config(&[("disabled", "no"), ("forbid_not_trusted", "yes")]),
            vec!["https://glpi.example/front/inventory.php".to_owned()],
            Arc::new(TrustList::default()),
            Arc::new(forwarder.clone()),
        );
        // 8.8.8.8 is not in the (default-empty + loopback) trust list.
        let res = post(app(state, [8, 8, 8, 8]), None, r#"{"deviceid":"host-1"}"#).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        assert!(forwarder.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stores_locally_without_forwarding() {
        let dir = tempfile::tempdir().unwrap();
        let forwarder = RecordingForwarder::default();
        let state = ProxyState::new(
            config(&[
                ("disabled", "no"),
                ("only_local_store", "yes"),
                ("local_store", dir.path().to_str().unwrap()),
            ]),
            vec!["https://glpi.example/front/inventory.php".to_owned()],
            Arc::new(TrustList::default()),
            Arc::new(forwarder.clone()),
        );
        let res = post(app(state, [127, 0, 0, 1]), None, r#"{"deviceid":"host-1"}"#).await;
        assert_eq!(res.status(), StatusCode::OK);
        // Stored, not forwarded.
        assert!(forwarder.calls.lock().unwrap().is_empty());
        assert!(dir.path().join("host-1.json").exists());
    }

    #[test]
    fn reads_device_id_from_json_and_xml() {
        assert_eq!(
            device_id(br#"{"deviceid":"pc-42","content":{}}"#).as_deref(),
            Some("pc-42")
        );
        assert_eq!(
            device_id(b"<REQUEST><DEVICEID>pc-42</DEVICEID></REQUEST>").as_deref(),
            Some("pc-42")
        );
        // Path separators in an id are sanitized away.
        assert_eq!(
            device_id(br#"{"deviceid":"a/b\\c"}"#).as_deref(),
            Some("a_b__c")
        );
    }
}
