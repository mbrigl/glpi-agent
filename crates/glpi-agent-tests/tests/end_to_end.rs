// SPDX-License-Identifier: GPL-2.0-only

//! Cross-crate integration + schema-parity tests (migration plan §13.4 #5).
//!
//! Each test drives several crates together against an in-memory mock, with no
//! network access:
//!
//! * a mock vSphere SOAP endpoint feeding the ESX task, whose inventories are
//!   then submitted to a mock GLPI server (vsphere → transport → protocol);
//! * a hand-built local inventory submitted to the mock GLPI server, asserting
//!   the GLPI wire schema;
//! * an SNMP `snmpwalk` capture run through the MIB registry, asserting the
//!   NetInventory device shape.

use glpi_core::protocol::glpi::InventoryRequest;
use glpi_discovery::{MibRegistry, SysObjectIds, WalkSession};
use glpi_inventory_local::content::VERSION_CLIENT;
use glpi_inventory_local::{Content, Hardware};
use glpi_transport::GlpiClient;
use glpi_vsphere::{EsxOptions, EsxTask, MockTransport};
use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ENDPOINT_PATH: &str = "/front/inventory.php";

/// Starts a mock GLPI server that accepts any inventory submission with
/// `{"status":"ok"}`, and returns its `/front/inventory.php` endpoint URL.
async fn mock_glpi_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ENDPOINT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"ok"})))
        .mount(&server)
        .await;
    server
}

fn endpoint(server: &MockServer) -> String {
    format!("{}{ENDPOINT_PATH}", server.uri())
}

// --- mock vSphere SOAP responses -------------------------------------------

const SERVICE_CONTENT: &str = "<RetrieveServiceContentResponse xmlns=\"urn:vim25\"><returnval>\
    <rootFolder type=\"Folder\">ha-folder-root</rootFolder>\
    <propertyCollector type=\"PropertyCollector\">ha-pc</propertyCollector>\
    <sessionManager type=\"SessionManager\">ha-sm</sessionManager>\
    <about><apiType>HostAgent</apiType></about></returnval></RetrieveServiceContentResponse>";
const LOGIN_OK: &str =
    "<LoginResponse xmlns=\"urn:vim25\"><returnval><key>s-1</key></returnval></LoginResponse>";
const PROPERTIES: &str = "<RetrievePropertiesResponse xmlns=\"urn:vim25\"><returnval>\
    <obj type=\"HostSystem\">host-1</obj>\
    <propSet><name>config.network.dnsConfig.hostName</name><val xsi:type=\"xsd:string\">esx1.lab</val></propSet>\
    <propSet><name>config.product.fullName</name><val xsi:type=\"xsd:string\">VMware ESXi 7.0.3 build-1</val></propSet>\
    <propSet><name>hardware.systemInfo.vendor</name><val xsi:type=\"xsd:string\">Dell Inc.</val></propSet>\
    <propSet><name>hardware.memorySize</name><val xsi:type=\"xsd:long\">34359738368</val></propSet>\
    </returnval></RetrievePropertiesResponse>";
const LOGOUT_OK: &str = "<LogoutResponse xmlns=\"urn:vim25\"></LogoutResponse>";

#[tokio::test]
async fn esx_soap_flow_inventory_reaches_mock_glpi_server() {
    // 1. ESX task drives the mock vSphere SOAP endpoint end to end.
    let transport = MockTransport::new([
        SERVICE_CONTENT.to_owned(),
        LOGIN_OK.to_owned(),
        PROPERTIES.to_owned(),
        LOGOUT_OK.to_owned(),
    ]);
    let task = EsxTask::new(
        "esx.lab",
        "root",
        "secret",
        EsxOptions {
            glpi_version: Some("10.0.17".to_owned()),
            ..EsxOptions::default()
        },
    );
    let hosts = task.collect_hosts_with(&transport).await.unwrap();
    let inventories = task.inventories(&hosts);
    assert_eq!(inventories.len(), 1);

    // 2. Each host inventory is submitted to the mock GLPI server.
    let server = mock_glpi_server().await;
    let client = GlpiClient::new(&endpoint(&server)).unwrap();
    for inv in &inventories {
        let request =
            InventoryRequest::new(inv.deviceid.clone(), &inv.content).with_itemtype(&inv.itemtype);
        client.submit_inventory(&request).await.unwrap();
    }

    // 3. The server received exactly the submission we expect.
    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let body: Value = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body["action"], "inventory");
    assert_eq!(body["deviceid"], "esx1.lab");
    assert_eq!(body["itemtype"], "Computer");
    // The vSphere total-RAM estimate survived the whole chain to the wire.
    assert_eq!(body["content"]["memories"][0]["capacity"], 32768);
    assert_eq!(body["content"]["bios"]["smanufacturer"], "Dell Inc.");
}

#[tokio::test]
async fn local_inventory_keeps_glpi_schema_over_the_wire() {
    let content = Content {
        version_client: Some(VERSION_CLIENT.to_owned()),
        hardware: Some(Hardware {
            name: Some("host-01".to_owned()),
            uuid: Some("uuid-1".to_owned()),
            vm_system: None,
        }),
        ..Content::default()
    };

    let server = mock_glpi_server().await;
    let client = GlpiClient::new(&endpoint(&server)).unwrap();
    let request = InventoryRequest::new("host-01", &content);
    client.submit_inventory(&request).await.unwrap();

    let received = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&received[0].body).unwrap();
    // GLPI requires `versionclient` and the lower-case `hardware` section.
    assert_eq!(body["content"]["versionclient"], VERSION_CLIENT);
    assert_eq!(body["content"]["hardware"]["name"], "host-01");
}

const SNMP_WALK: &str = r#".1.3.6.1.2.1.1.1.0 = STRING: "Cisco IOS Software, C2960"
.1.3.6.1.2.1.1.2.0 = OID: .1.3.6.1.4.1.9.1.3
.1.3.6.1.2.1.1.3.0 = Timeticks: (123456789) 14 days, 6:56:07.89
.1.3.6.1.2.1.1.4.0 = STRING: "netops@example.com"
.1.3.6.1.2.1.1.5.0 = STRING: "core-sw-1"
.1.3.6.1.2.1.1.6.0 = STRING: "Rack 4"
"#;

#[tokio::test]
async fn snmp_walk_drives_netinventory_device_shape() {
    let mut walk = WalkSession::parse(SNMP_WALK).unwrap();
    let device = MibRegistry::with_defaults()
        .inventory(&mut walk, &SysObjectIds::default())
        .await
        .unwrap();

    assert_eq!(
        device.info.description.as_deref(),
        Some("Cisco IOS Software, C2960")
    );
    assert_eq!(device.info.name.as_deref(), Some("core-sw-1"));
    assert_eq!(device.info.contact.as_deref(), Some("netops@example.com"));
    assert_eq!(device.info.location.as_deref(), Some("Rack 4"));

    // Serializes into the NetInventory result schema.
    let json = serde_json::to_value(&device).unwrap();
    assert!(json["info"]["description"].is_string());
    assert!(json["ports"].is_array());
}
