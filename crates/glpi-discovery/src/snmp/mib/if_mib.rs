// SPDX-License-Identifier: GPL-2.0-only

//! Standard `IF-MIB` interface support.
//!
//! Walks `ifTable` (RFC 1213) and the `ifXTable` extensions (RFC 2863) to build
//! the device's [`Port`] list: index, name/description/alias, type, MTU, speed,
//! MAC and admin/oper status. Speed prefers `ifHighSpeed` (Mbit/s) when present,
//! since `ifSpeed` saturates at ~4.29 Gbit/s.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use super::{apply_column, as_i64, as_mac, as_u64, MibSupport, NetworkDevice, Port};
use crate::snmp::query::SnmpQuery;

// ifTable columns (1.3.6.1.2.1.2.2.1.N).
const IF_DESCR: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 2];
const IF_TYPE: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 3];
const IF_MTU: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 4];
const IF_SPEED: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 5];
const IF_PHYS_ADDRESS: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 6];
const IF_ADMIN_STATUS: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 7];
const IF_OPER_STATUS: [u64; 10] = [1, 3, 6, 1, 2, 1, 2, 2, 1, 8];
// ifXTable columns (1.3.6.1.2.1.31.1.1.1.N).
const IF_NAME: [u64; 11] = [1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 1];
const IF_HIGH_SPEED: [u64; 11] = [1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 15];
const IF_ALIAS: [u64; 11] = [1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 18];

/// MIB module for the standard interface tables.
#[derive(Debug, Default, Clone, Copy)]
pub struct IfMib;

#[async_trait]
impl MibSupport for IfMib {
    fn name(&self) -> &'static str {
        "if"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let mut ports: BTreeMap<u64, Port> = BTreeMap::new();

        apply_column(session, &IF_DESCR, &mut ports, Port::new, |port, v| {
            port.description = v.as_str();
        })
        .await?;
        apply_column(session, &IF_TYPE, &mut ports, Port::new, |port, v| {
            port.if_type = as_i64(&v);
        })
        .await?;
        apply_column(session, &IF_MTU, &mut ports, Port::new, |port, v| {
            port.mtu = as_i64(&v);
        })
        .await?;
        apply_column(session, &IF_SPEED, &mut ports, Port::new, |port, v| {
            port.speed = as_u64(&v);
        })
        .await?;
        apply_column(
            session,
            &IF_PHYS_ADDRESS,
            &mut ports,
            Port::new,
            |port, v| {
                port.mac = as_mac(&v);
            },
        )
        .await?;
        apply_column(
            session,
            &IF_ADMIN_STATUS,
            &mut ports,
            Port::new,
            |port, v| {
                port.admin_status = as_i64(&v);
            },
        )
        .await?;
        apply_column(
            session,
            &IF_OPER_STATUS,
            &mut ports,
            Port::new,
            |port, v| {
                port.oper_status = as_i64(&v);
            },
        )
        .await?;
        apply_column(session, &IF_NAME, &mut ports, Port::new, |port, v| {
            port.name = v.as_str();
        })
        .await?;
        apply_column(session, &IF_ALIAS, &mut ports, Port::new, |port, v| {
            port.alias = v.as_str();
        })
        .await?;
        // ifHighSpeed (Mbit/s) overrides ifSpeed when present and non-zero.
        apply_column(session, &IF_HIGH_SPEED, &mut ports, Port::new, |port, v| {
            if let Some(mbps) = as_u64(&v).filter(|m| *m > 0) {
                port.speed = Some(mbps * 1_000_000);
            }
        })
        .await?;

        device.ports = ports.into_values().collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::IfMib;
    use crate::snmp::mib::{MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;
    use glpi_core::types::network::MacAddress;

    const IF_WALK: &str = r#".1.3.6.1.2.1.2.2.1.1.1 = INTEGER: 1
.1.3.6.1.2.1.2.2.1.1.2 = INTEGER: 2
.1.3.6.1.2.1.2.2.1.2.1 = STRING: "GigabitEthernet0/1"
.1.3.6.1.2.1.2.2.1.2.2 = STRING: "GigabitEthernet0/2"
.1.3.6.1.2.1.2.2.1.3.1 = INTEGER: 6
.1.3.6.1.2.1.2.2.1.4.1 = INTEGER: 1500
.1.3.6.1.2.1.2.2.1.5.1 = Gauge32: 1000000000
.1.3.6.1.2.1.2.2.1.6.1 = Hex-STRING: 00 1A 2B 3C 4D 5E
.1.3.6.1.2.1.2.2.1.6.2 = Hex-STRING: 00 00 00 00 00 00
.1.3.6.1.2.1.2.2.1.7.1 = INTEGER: 1
.1.3.6.1.2.1.2.2.1.8.1 = INTEGER: 1
.1.3.6.1.2.1.2.2.1.8.2 = INTEGER: 2
.1.3.6.1.2.1.31.1.1.1.1.1 = STRING: "Gi0/1"
.1.3.6.1.2.1.31.1.1.1.15.2 = Gauge32: 10000
.1.3.6.1.2.1.31.1.1.1.18.1 = STRING: "uplink to core"
"#;

    async fn ports() -> Vec<crate::snmp::mib::device::Port> {
        let mut session = WalkSession::parse(IF_WALK).unwrap();
        let mut device = NetworkDevice::default();
        IfMib.run(&mut session, &mut device).await.unwrap();
        device.ports
    }

    #[tokio::test]
    async fn builds_ports_ordered_by_index() {
        let ports = ports().await;
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].index, 1);
        assert_eq!(ports[1].index, 2);
    }

    #[tokio::test]
    async fn populates_port_one_fields() {
        let ports = ports().await;
        let p = &ports[0];
        assert_eq!(p.name.as_deref(), Some("Gi0/1"));
        assert_eq!(p.description.as_deref(), Some("GigabitEthernet0/1"));
        assert_eq!(p.alias.as_deref(), Some("uplink to core"));
        assert_eq!(p.if_type, Some(6));
        assert_eq!(p.mtu, Some(1500));
        assert_eq!(p.speed, Some(1_000_000_000));
        assert_eq!(
            p.mac,
            Some(MacAddress::new([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]))
        );
        assert_eq!(p.admin_status, Some(1));
        assert_eq!(p.oper_status, Some(1));
    }

    #[tokio::test]
    async fn high_speed_overrides_and_zero_mac_dropped() {
        let ports = ports().await;
        let p = &ports[1];
        // ifHighSpeed 10000 Mbit/s -> 10 Gbit/s; no ifSpeed for this port.
        assert_eq!(p.speed, Some(10_000_000_000));
        // All-zero ifPhysAddress is not a usable MAC.
        assert_eq!(p.mac, None);
        assert_eq!(p.oper_status, Some(2));
    }
}
