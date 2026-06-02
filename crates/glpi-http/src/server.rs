// SPDX-License-Identifier: GPL-2.0-only

//! The embedded HTTP control server.
//!
//! Exposes the agent's control endpoints on `httpd-port` (default 62354):
//!
//! * `GET /status` — a plain-text liveness/status line for the GLPI server;
//! * `GET|POST /now` — trigger an immediate run, with the `partial`, `full`,
//!   `task`, `category` and `delay` query parameters; the request is mapped to a
//!   typed [`Event`] and sent to the daemon over a channel;
//! * `GET /` — a short index of the available endpoints.
//!
//! Every request is gated by the [`TrustList`]: untrusted clients get `403`.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::StatusCode;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{extract::Request, Router};
use glpi_core::error::{AgentError, Result};
use glpi_scheduler::Event;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::proxy::{InventoryForwarder, ProxyState, TransportForwarder};
use crate::trust::TrustList;
use glpi_plugins::proxy::ProxyConfig;

/// Default port of the embedded HTTP server.
pub const DEFAULT_HTTP_PORT: u16 = 62354;

/// Raw `/now` query string.
#[derive(Debug, Default, Deserialize)]
struct NowQuery {
    partial: Option<String>,
    full: Option<String>,
    task: Option<String>,
    category: Option<String>,
    delay: Option<u64>,
}

/// `?flag`, `?flag=yes`, `=1`, `=true` are truthy; `=no`/absent are not.
fn truthy(value: &Option<String>) -> bool {
    matches!(value.as_deref(), Some("" | "yes" | "1" | "true"))
}

impl NowQuery {
    /// Maps a `/now` request to a typed agent [`Event`]: a `partial` request
    /// becomes a partial-inventory event, otherwise a `runnow` for the given
    /// task (or all tasks). `full` and `delay` are carried through.
    fn into_event(self) -> Event {
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        if let Some(delay) = self.delay {
            params.insert("delay".to_owned(), delay.to_string());
        }
        if let Some(full) = self.full {
            params.insert("full".to_owned(), full);
        }
        if truthy(&self.partial) {
            params.insert("partial".to_owned(), "1".to_owned());
            if let Some(category) = self.category {
                params.insert("category".to_owned(), category);
            }
        } else {
            params.insert("runnow".to_owned(), "1".to_owned());
            if let Some(task) = self.task {
                params.insert("task".to_owned(), task);
            }
        }
        // `from_params` always succeeds here (a kind flag is set); fall back to
        // a plain run-now for safety.
        Event::from_params(&params).unwrap_or_else(|| Event::run_now("", 0, BTreeMap::new()))
    }
}

/// Shared handler state.
#[derive(Clone)]
struct ServerState {
    trust: Arc<TrustList>,
    triggers: mpsc::Sender<Event>,
    status_line: Arc<str>,
}

/// The embedded control server.
pub struct HttpServer {
    ip: IpAddr,
    port: u16,
    state: ServerState,
    proxy: Option<ProxyState>,
}

impl HttpServer {
    /// Builds a server bound to `ip:port`, trusting `trust`, and reporting
    /// `status_line` on `/status`. Returns the server and the receiver that
    /// yields an [`Event`] each time `/now` is hit.
    #[must_use]
    pub fn new(
        ip: IpAddr,
        port: u16,
        trust: TrustList,
        status_line: impl Into<Arc<str>>,
    ) -> (Self, mpsc::Receiver<Event>) {
        let (triggers, rx) = mpsc::channel(32);
        let state = ServerState {
            trust: Arc::new(trust),
            triggers,
            status_line: status_line.into(),
        };
        (
            Self {
                ip,
                port,
                state,
                proxy: None,
            },
            rx,
        )
    }

    /// Mounts the Proxy server plugin on `config.url_path`, relaying received
    /// inventories to `servers` (and/or a local store) via the production
    /// `glpi-transport` forwarder. The proxy applies its own trust policy.
    #[must_use]
    pub fn with_proxy(self, config: ProxyConfig, servers: Vec<String>) -> Self {
        self.with_proxy_forwarder(config, servers, Arc::new(TransportForwarder))
    }

    /// Like [`with_proxy`](Self::with_proxy) but with a caller-supplied
    /// forwarder (used by tests to record forwards without a network).
    #[must_use]
    pub fn with_proxy_forwarder(
        mut self,
        config: ProxyConfig,
        servers: Vec<String>,
        forwarder: Arc<dyn InventoryForwarder>,
    ) -> Self {
        self.proxy = Some(ProxyState::new(
            config,
            servers,
            self.state.trust.clone(),
            forwarder,
        ));
        self
    }

    /// Builds the router (exposed for testing).
    pub fn router(&self) -> Router {
        let mut app = Router::new()
            .route("/status", get(status))
            .route("/now", get(now).post(now))
            .route("/", get(index))
            .layer(from_fn_with_state(self.state.clone(), enforce_trust))
            .with_state(self.state.clone());
        if let Some(proxy) = &self.proxy {
            app = app.merge(crate::proxy::router(proxy.clone()));
        }
        app
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
    let event = query.into_event();
    match state.triggers.send(event).await {
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
    use super::{HttpServer, NowQuery, DEFAULT_HTTP_PORT};
    use crate::trust::TrustList;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::{Request, StatusCode};
    use glpi_scheduler::EventKind;
    use std::net::SocketAddr;
    use tower::ServiceExt;

    fn server(
        trust: TrustList,
    ) -> (
        HttpServer,
        tokio::sync::mpsc::Receiver<glpi_scheduler::Event>,
    ) {
        HttpServer::new(
            "127.0.0.1".parse().unwrap(),
            DEFAULT_HTTP_PORT,
            trust,
            "glpi-agent test",
        )
    }

    #[test]
    fn now_partial_request_becomes_partial_event() {
        let event = NowQuery {
            partial: Some("yes".to_owned()),
            category: Some("cpu,memory".to_owned()),
            ..NowQuery::default()
        }
        .into_event();
        assert_eq!(event.kind, EventKind::Partial);
        assert_eq!(event.task, "inventory");
        assert_eq!(event.category, "cpu,memory");
    }

    #[test]
    fn now_default_becomes_runnow_with_task_and_delay() {
        let event = NowQuery {
            task: Some("inventory".to_owned()),
            full: Some("1".to_owned()),
            delay: Some(5),
            ..NowQuery::default()
        }
        .into_event();
        assert_eq!(event.kind, EventKind::RunNow);
        assert_eq!(event.task, "inventory");
        assert_eq!(event.delay, 5);
        assert_eq!(event.get("full"), Some("1"));
    }

    #[test]
    fn now_empty_becomes_runnow_all() {
        let event = NowQuery::default().into_event();
        assert_eq!(event.kind, EventKind::RunNow);
        assert_eq!(event.task, "all");
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
                    .uri("/now?task=netdiscovery&delay=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let event = rx.try_recv().expect("a trigger event");
        assert_eq!(event.kind, EventKind::RunNow);
        assert_eq!(event.task, "netdiscovery");
        assert_eq!(event.delay, 3);
    }
}
