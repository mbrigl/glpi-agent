// SPDX-License-Identifier: GPL-2.0-only

//! Network inventory category (Linux `ip -o link` + `ip -o addr`).
//!
//! Interfaces come from `ip -o link show` (name, MAC, MTU, up/down) and their
//! addresses from `ip -o addr show`, merged by interface name. The `-o`
//! (one-line) form keeps each record on a single line, which the pure
//! [`parse_interfaces`] parser handles; the live collector runs both commands
//! and enriches link speed from sysfs.

use std::collections::HashMap;
use std::net::IpAddr;

use glpi_core::types::network::MacAddress;
use serde::{Serialize, Serializer};

/// A network interface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NetworkInterface {
    /// Interface name (e.g. "eth0").
    #[serde(rename = "description")]
    pub name: String,
    /// Hardware (MAC) address, if any.
    #[serde(rename = "mac", skip_serializing_if = "Option::is_none")]
    pub mac: Option<MacAddress>,
    /// Assigned IP addresses (v4 and v6). GLPI's `ipaddress` is a single
    /// string, so serialization emits the primary (first) address; the full
    /// list is retained in the struct (multi-IP submission is a refinement).
    #[serde(
        rename = "ipaddress",
        serialize_with = "serialize_primary_ip",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub ips: Vec<IpAddr>,
    /// MTU.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,
    /// "up" or "down".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Link speed in Mbit/s (from sysfs), if known. GLPI expects a string.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt_number_as_string"
    )]
    pub speed: Option<u64>,
}

/// Serializes an optional number as GLPI's string form (skip handles `None`).
fn serialize_opt_number_as_string<S: Serializer>(
    value: &Option<u64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(n) => serializer.collect_str(n),
        None => serializer.serialize_none(),
    }
}

/// Serializes the IP list as GLPI's single `ipaddress` string (the primary
/// address). The field's `skip_serializing_if` guarantees a non-empty list.
#[allow(clippy::ptr_arg)] // serde passes the field as &Vec<IpAddr>.
fn serialize_primary_ip<S: Serializer>(
    ips: &Vec<IpAddr>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&ips[0])
}

/// Parses `ip -o link show` and `ip -o addr show` into interfaces.
#[must_use]
pub fn parse_interfaces(link_output: &str, addr_output: &str) -> Vec<NetworkInterface> {
    let mut interfaces: Vec<NetworkInterface> =
        link_output.lines().filter_map(parse_link_line).collect();

    let index: HashMap<String, usize> = interfaces
        .iter()
        .enumerate()
        .map(|(i, iface)| (iface.name.clone(), i))
        .collect();

    for line in addr_output.lines() {
        if let Some((name, ip)) = parse_addr_line(line) {
            if let Some(&pos) = index.get(&name) {
                interfaces[pos].ips.push(ip);
            }
        }
    }
    interfaces
}

/// Collects the live interfaces, enriching speed from sysfs (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<NetworkInterface> {
    let (Some(link), Some(addr)) = (
        run_ip(&["-o", "link", "show"]),
        run_ip(&["-o", "addr", "show"]),
    ) else {
        return Vec::new();
    };
    let mut interfaces = parse_interfaces(&link, &addr);
    for iface in &mut interfaces {
        iface.speed = std::fs::read_to_string(format!("/sys/class/net/{}/speed", iface.name))
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .filter(|s| *s > 0)
            .map(|s| s as u64);
    }
    interfaces
}

/// Collects the live interfaces (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<NetworkInterface> {
    Vec::new()
}

/// Runs `ip <args>`, returning stdout on success.
#[cfg(target_os = "linux")]
fn run_ip(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("ip").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parses one `ip -o link show` line.
fn parse_link_line(line: &str) -> Option<NetworkInterface> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    // tokens[0] = "2:", tokens[1] = "eth0:" (or "eth0@if49:" for veth pairs —
    // `ip addr` reports the bare name, so strip the "@peer" suffix to match).
    let raw = tokens.get(1)?.trim_end_matches(':');
    let name = raw.split('@').next().unwrap_or(raw).to_owned();
    if name.is_empty() {
        return None;
    }

    let mtu = token_after(&tokens, "mtu").and_then(|v| v.parse().ok());
    let mac = tokens
        .iter()
        .position(|t| t.starts_with("link/"))
        .and_then(|i| tokens.get(i + 1))
        .and_then(|m| m.parse::<MacAddress>().ok())
        .filter(|m| m.octets() != [0u8; 6]);
    let status = link_status(&tokens);

    Some(NetworkInterface {
        name,
        mac,
        mtu,
        status,
        ..NetworkInterface::default()
    })
}

/// Parses one `ip -o addr show` line into `(interface, address)`.
fn parse_addr_line(line: &str) -> Option<(String, IpAddr)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let name = tokens.get(1)?.trim_end_matches(':').to_owned();
    let family = tokens.iter().position(|t| *t == "inet" || *t == "inet6")?;
    let cidr = tokens.get(family + 1)?;
    let addr = cidr.split('/').next()?;
    addr.parse::<IpAddr>().ok().map(|ip| (name, ip))
}

/// Returns "up"/"down" from the interface flags (`<…,UP,…>`).
fn link_status(tokens: &[&str]) -> Option<String> {
    let flags = tokens
        .iter()
        .find(|t| t.starts_with('<') && t.ends_with('>'))?;
    let up = flags
        .trim_matches(['<', '>'])
        .split(',')
        .any(|flag| flag == "UP");
    Some(if up { "up" } else { "down" }.to_owned())
}

/// Returns the token following the first occurrence of `key`.
fn token_after<'a>(tokens: &[&'a str], key: &str) -> Option<&'a str> {
    let pos = tokens.iter().position(|t| *t == key)?;
    tokens.get(pos + 1).copied()
}

#[cfg(test)]
mod tests {
    use super::parse_interfaces;
    use glpi_core::types::network::MacAddress;

    const LINK: &str = "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN mode DEFAULT group default qlen 1000    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
2: eth0@if49: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc fq_codel state UP mode DEFAULT group default qlen 1000    link/ether 02:42:ac:11:00:02 brd ff:ff:ff:ff:ff:ff
3: down0: <BROADCAST,MULTICAST> mtu 1500 qdisc noop state DOWN mode DEFAULT group default qlen 1000    link/ether 0a:0b:0c:0d:0e:0f brd ff:ff:ff:ff:ff:ff
";

    const ADDR: &str = "1: lo    inet 127.0.0.1/8 scope host lo
2: eth0    inet 172.17.0.2/16 brd 172.17.255.255 scope global eth0
2: eth0    inet6 fe80::42:acff:fe11:2/64 scope link
";

    #[test]
    fn merges_links_and_addresses() {
        let ifaces = parse_interfaces(LINK, ADDR);
        assert_eq!(ifaces.len(), 3);

        let lo = &ifaces[0];
        assert_eq!(lo.name, "lo");
        assert_eq!(lo.mac, None); // all-zero loopback MAC dropped
        assert_eq!(lo.mtu, Some(65536));
        assert_eq!(lo.status.as_deref(), Some("up"));
        assert_eq!(
            lo.ips,
            vec!["127.0.0.1".parse::<std::net::IpAddr>().unwrap()]
        );

        // Link name is "eth0@if49" (veth); the "@peer" suffix is stripped so
        // the bare "eth0" from `ip addr` merges its IPs in.
        let eth0 = &ifaces[1];
        assert_eq!(eth0.name, "eth0");
        assert_eq!(
            eth0.mac,
            Some(MacAddress::new([0x02, 0x42, 0xac, 0x11, 0x00, 0x02]))
        );
        assert_eq!(eth0.status.as_deref(), Some("up"));
        assert_eq!(eth0.ips.len(), 2); // v4 + v6 merged despite the @if49 suffix
    }

    #[test]
    fn down_interface_status() {
        let ifaces = parse_interfaces(LINK, "");
        let down = ifaces.iter().find(|i| i.name == "down0").unwrap();
        assert_eq!(down.status.as_deref(), Some("down"));
        assert!(down.ips.is_empty());
    }

    #[test]
    fn empty_input_yields_no_interfaces() {
        assert!(parse_interfaces("", "").is_empty());
    }
}
