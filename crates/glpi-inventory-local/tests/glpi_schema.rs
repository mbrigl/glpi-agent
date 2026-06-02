// SPDX-License-Identifier: GPL-2.0-only

//! Regression guard for GLPI inventory-schema parity.
//!
//! These assertions lock the content shapes that were validated against a live
//! GLPI server (it accepted the inventory with `{"RESPONSE":"SEND"}`). They
//! exist so the schema-critical serialization can't silently regress:
//!
//! * `content.versionclient` is present (a required string);
//! * `operatingsystem.timezone` carries both `name` and `offset`;
//! * `networks[].ipaddress` is a single string (not an array);
//! * `networks[].status` is the lower-case enum (`up`/`down`/…);
//! * `networks[].speed` is serialized as a string;
//! * `processes[]` does not emit `started` (GLPI requires an absolute
//!   timestamp the `ps` START column can't supply).

use std::net::IpAddr;

use glpi_inventory_local::content::VERSION_CLIENT;
use glpi_inventory_local::{parse_ps, Content, NetworkInterface, OperatingSystem, Timezone};
use serde_json::Value;

/// Builds a representative inventory exercising the schema-sensitive sections.
fn sample_content() -> Content {
    let network = NetworkInterface {
        name: "eth0".to_owned(),
        mac: None,
        ips: vec![
            "10.0.0.1".parse::<IpAddr>().unwrap(),
            "10.0.0.2".parse::<IpAddr>().unwrap(),
        ],
        mtu: Some(1500),
        status: Some("up".to_owned()),
        speed: Some(1000),
    };
    let operating_system = OperatingSystem {
        name: Some("Debian GNU/Linux".to_owned()),
        timezone: Some(Timezone {
            name: "Europe/Berlin".to_owned(),
            offset: Some("+0100".to_owned()),
        }),
        ..OperatingSystem::default()
    };
    Content {
        version_client: Some(VERSION_CLIENT.to_owned()),
        operating_system: Some(operating_system),
        networks: vec![network],
        processes: parse_ps(
            "USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND\n\
             root 1 0.0 0.1 1000 100 ? Ss 10:00 0:01 /sbin/init\n",
        ),
        ..Content::default()
    }
}

#[test]
fn inventory_content_matches_glpi_schema_shapes() {
    let json: Value = serde_json::to_value(sample_content()).unwrap();

    // Required agent identifier.
    assert!(
        json["versionclient"].is_string(),
        "versionclient must be a string"
    );

    // Timezone needs both name and offset.
    let tz = &json["operatingsystem"]["timezone"];
    assert!(tz["name"].is_string(), "timezone.name must be a string");
    assert!(tz["offset"].is_string(), "timezone.offset must be present");

    // Network: ipaddress is a single string (primary), status is the lower-case
    // enum, speed is a string.
    let net = &json["networks"][0];
    assert_eq!(net["ipaddress"], Value::from("10.0.0.1"));
    assert!(net["ipaddress"].is_string(), "ipaddress must be a string");
    assert_eq!(net["status"], Value::from("up"));
    assert_eq!(net["speed"], Value::from("1000"));
    assert!(net["speed"].is_string(), "speed must be a string");

    // Processes must not carry `started`.
    let proc = &json["processes"][0];
    assert_eq!(proc["pid"], Value::from(1));
    assert!(
        proc.get("started").is_none(),
        "processes.started must be omitted"
    );
}

#[test]
fn empty_partial_inventory_still_carries_versionclient() {
    // A fully-filtered run must still be a valid submission shell.
    let content = Content {
        version_client: Some(VERSION_CLIENT.to_owned()),
        ..Content::default()
    };
    let json = serde_json::to_value(&content).unwrap();
    assert!(json["versionclient"].is_string());
    // Nothing else is emitted.
    assert_eq!(json.as_object().unwrap().len(), 1);
}
