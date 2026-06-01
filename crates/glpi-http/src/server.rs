// SPDX-License-Identifier: GPL-2.0-only

//! The embedded HTTP control server.
//!
//! Exposes the agent's control endpoints on `httpd-port` (default 62354):
//!
//! * `GET /status` — a plain-text liveness/status line for the GLPI server;
//! * `GET|POST /now` — trigger an immediate run, with the `partial`, `full`,
//!   `task` and `delay` query parameters; the parsed [`NowRequest`] is sent to
//!   the daemon over a channel;
//! * `GET /` — a short index of the available endpoints.
//!
//! Every request is gated by the [`TrustList`]: untrusted clients get `403`.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::StatusCode;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{extract::Request, Router};
use glpi_core::error::{AgentError, Result};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::trust::TrustList;

/// Default port of the embedded HTTP server.
pub const DEFAULT_HTTP_PORT: u16 = 62354;

/// A parsed `/now` trigger request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NowRequest {
    /// Run a partial inventory.
    pub partial: bool,
    /// Force a full inventory.
    pub full: bool,
    /// Restrict to specific tasks (comma-separated), if given.
    pub tasks: Option<String>,
    /// Delay the run by this many seconds, if given.
    pub delay: Option<u64>,
}

/// Raw `/now` query string, before normalization.
#[derive(Debug, Default, Deserialize)]
struct NowQuery {
    partial: Option<String>,
    full: Option<String>,
    task: Option<String>,
    delay: Option<u64>,
}

/// `?flag`, `?flag=yes`, `=1`, `=true` are truthy; `=no`/absent are not.
fn truthy(value: &Option<String>) -> bool {
    matches!(value.as_deref(), Some("" | "yes" | "1" | "true"))
}

impl From<NowQuery> for NowRequest {
    fn from(q: NowQuery) -> Self {
        Self {
            partial: truthy(&q.partial),
            full: truthy(&q.full),
            tasks: q.task,
            delay: q.delay,
        }
    }
}

/// Shared handler state.
#[derive(Clone)]
struct ServerState {
    trust: Arc<TrustList>,
    triggers: mpsc::Sender<NowRequest>,
    status_line: Arc<str>,
}

/// The embedded control server.
pub struct HttpServer {
    ip: IpAddr,
    port: u16,
    state: ServerState,
}

impl HttpServer {
    /// Builds a server bound to `ip:port`, trusting `trust`, and reporting
    /// `status_line` on `/status`. Returns the server and the receiver that
    /// yields a [`NowRequest`] each time `/now` is hit.
    #[must_use]
    pub fn new(
        ip: IpAddr,
        port: u16,
        trust: TrustList,
        status_line: impl Into<Arc<str>>,
    ) -> (Self, mpsc::Receiver<NowRequest>) {
        let (triggers, rx) = mpsc::channel(32);
        let state = ServerState {
            trust: Arc::new(trust),
            triggers,
            status_line: status_line.into(),
        };
        (Self { ip, port, state }, rx)
    }

    /// Builds the router (exposed for testing).
    pub fn router(&self) -> Router {
        Router::new()
            .route("/status", get(status))
            .route("/now", get(now).post(now))
            .route("/", get(index))
            .layer(from_fn_with_state(self.state.clone(), enforce_trust))
            .with_state(self.state.clone())
    }

    /// Binds the socket and serves until the process ends.
    ///
    /// # Errors
    ///
    /// Returns an error if the listen socket cannot be bound or the server
    /// loop fails.
    pub async fn serve(self) -> Result<()> {
        let addr = SocketAddr::new(self.ip, self.port);
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            AgentError::Transport(format!("cannot bind HTTP server on {addr}: {e}"))
        })?;
        tracing::info!(%addr, "HTTP control server listening");
        axum::serve(
            listener,
            self.router()
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| AgentError::Transport(format!("HTTP server error: {e}")))
    }
}

/// Trust middleware: rejects clients not allowed by the [`TrustList`].
async fn enforce_trust(
    State(state): State<ServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if state.trust.allows(addr.ip()) {
        next.run(request).await
    } else {
        tracing::debug!(client = %addr.ip(), "rejected untrusted HTTP client");
        (StatusCode::FORBIDDEN, "forbidden\n").into_response()
    }
}

async fn status(State(state): State<ServerState>) -> impl IntoResponse {
    (StatusCode::OK, format!("{}\n", state.status_line))
}

async fn now(State(state): State<ServerState>, Query(query): Query<NowQuery>) -> impl IntoResponse {
    let request = NowRequest::from(query);
    match state.triggers.send(request).await {
        Ok(()) => (StatusCode::OK, "running now\n"),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "agent not accepting events\n",
        ),
    }
}

async fn index() -> impl IntoResponse {
    (StatusCode::OK, "GLPI Agent\nEndpoints: /status, /now\n")
}

#[cfg(test)]
mod tests {
    use super::{HttpServer, NowQuery, NowRequest, DEFAULT_HTTP_PORT};
    use crate::trust::TrustList;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::{Request, StatusCode};
    use std::net::SocketAddr;
    use tower::ServiceExt;

    fn server(trust: TrustList) -> (HttpServer, tokio::sync::mpsc::Receiver<NowRequest>) {
        HttpServer::new(
            "127.0.0.1".parse().unwrap(),
            DEFAULT_HTTP_PORT,
            trust,
            "glpi-agent test",
        )
    }

    #[test]
    fn now_query_truthiness() {
        let q = NowQuery {
            partial: Some("yes".to_owned()),
            full: Some("no".to_owned()),
            task: Some("inventory".to_owned()),
            delay: Some(5),
        };
        let req = NowRequest::from(q);
        assert!(req.partial);
        assert!(!req.full);
        assert_eq!(req.tasks.as_deref(), Some("inventory"));
        assert_eq!(req.delay, Some(5));
    }

    #[tokio::test]
    async fn status_allowed_for_trusted_client() {
        let (server, _rx) = server(TrustList::default());
        let app = server
            .router()
            .layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn untrusted_client_is_forbidden() {
        let (server, _rx) = server(TrustList::default());
        let app = server
            .router()
            .layer(MockConnectInfo(SocketAddr::from(([8, 8, 8, 8], 40000))));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn now_emits_a_trigger_event() {
        let (server, mut rx) = server(TrustList::default());
        let app = server
            .router()
            .layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/now?partial=yes&task=netdiscovery")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let event = rx.try_recv().expect("a trigger event");
        assert!(event.partial);
        assert_eq!(event.tasks.as_deref(), Some("netdiscovery"));
    }
}
