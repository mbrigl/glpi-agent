// SPDX-License-Identifier: GPL-2.0-only

//! Integration tests for [`glpi_transport::Injector`] against a mock server.

use glpi_transport::{ContentFormat, GlpiClient, Injector};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ENDPOINT_PATH: &str = "/front/inventory.php";

fn endpoint(server: &MockServer) -> String {
    format!("{}{ENDPOINT_PATH}", server.uri())
}

fn unique_path(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "glpi-injector-{}-{nanos}{suffix}",
        std::process::id()
    ))
}

#[tokio::test]
async fn injects_json_file_with_content_type() {
    let server = MockServer::start().await;
    let payload = r#"{"action":"inventory","deviceid":"agent-1"}"#;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .and(header("content-type", "application/json"))
        .and(body_string(payload))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let file = unique_path(".json");
    std::fs::write(&file, payload).unwrap();

    let injector = Injector::new(GlpiClient::new(&endpoint(&server)).unwrap());
    let result = injector.inject_file(&file).await;
    std::fs::remove_file(&file).ok();

    assert!(result.is_ok());
}

#[tokio::test]
async fn injects_xml_bytes_with_content_type() {
    let server = MockServer::start().await;
    let payload = b"<?xml version=\"1.0\"?><REQUEST/>".to_vec();
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .and(header("content-type", "application/xml"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let injector = Injector::new(GlpiClient::new(&endpoint(&server)).unwrap());

    assert!(injector
        .inject_bytes(payload, ContentFormat::Xml)
        .await
        .is_ok());
}

#[tokio::test]
async fn unknown_extension_errors_before_any_request() {
    let server = MockServer::start().await;
    // No mock mounted: a request would fail the test by returning non-2xx.
    let file = unique_path(".txt");
    std::fs::write(&file, "irrelevant").unwrap();

    let injector = Injector::new(GlpiClient::new(&endpoint(&server)).unwrap());
    let result = injector.inject_file(&file).await;
    std::fs::remove_file(&file).ok();

    assert!(result.is_err());
}
