// SPDX-License-Identifier: GPL-2.0-only

//! Integration tests for [`glpi_transport::GlpiClient`] against a mock server.

use glpi_core::error::AgentError;
use glpi_core::protocol::{ContactRequest, InventoryRequest};
use glpi_transport::GlpiClient;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ENDPOINT_PATH: &str = "/front/inventory.php";

fn endpoint(server: &MockServer) -> String {
    format!("{}{ENDPOINT_PATH}", server.uri())
}

#[tokio::test]
async fn contact_parses_task_plan() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok",
            "expiration": "24",
            "tasks": { "inventory": { "params": [] } },
        })))
        .mount(&server)
        .await;

    let client = GlpiClient::new(&endpoint(&server)).unwrap();
    let response = client
        .contact(&ContactRequest::new("agent-1"))
        .await
        .unwrap();

    assert_eq!(response.status.as_deref(), Some("ok"));
    assert!(response.tasks.contains_key("inventory"));
}

#[tokio::test]
async fn submit_inventory_succeeds_on_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "ok" })))
        .mount(&server)
        .await;

    let client = GlpiClient::new(&endpoint(&server)).unwrap();
    let request = InventoryRequest::new("agent-1", json!({ "hardware": { "name": "host" } }));

    assert!(client.submit_inventory(&request).await.is_ok());
}

#[tokio::test]
async fn server_error_maps_to_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = GlpiClient::new(&endpoint(&server)).unwrap();
    let err = client
        .contact(&ContactRequest::new("agent-1"))
        .await
        .unwrap_err();

    assert!(matches!(err, AgentError::Transport(_)));
}

#[tokio::test]
async fn unauthorized_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = GlpiClient::new(&endpoint(&server)).unwrap();
    let err = client
        .contact(&ContactRequest::new("agent-1"))
        .await
        .unwrap_err();

    assert!(matches!(err, AgentError::Auth(_)));
}

#[tokio::test]
async fn basic_auth_header_is_sent() {
    let server = MockServer::start().await;
    // base64("scout:secret") == "c2NvdXQ6c2VjcmV0"
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .and(header("authorization", "Basic c2NvdXQ6c2VjcmV0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "ok" })))
        .expect(1)
        .mount(&server)
        .await;

    let client = GlpiClient::new(&endpoint(&server))
        .unwrap()
        .with_basic_auth("scout", "secret");

    client
        .contact(&ContactRequest::new("agent-1"))
        .await
        .unwrap();
    // MockServer verifies the `expect(1)` matcher (including the auth header)
    // on drop.
}
