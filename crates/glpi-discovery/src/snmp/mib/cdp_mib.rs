// SPDX-License-Identifier: GPL-2.0-only

//! `CISCO-CDP-MIB` neighbor discovery (Cisco Discovery Protocol).
//!
//! Walks `cdpCacheTable` and attaches a [`Neighbor`] to each local port:
//! remote device id (system name), software version (description) and the
//! remote port. The table is indexed by `(cdpCacheIfIndex, cdpCacheDeviceIndex)`
//! where the first arc is the local `ifIndex` directly, so — unlike LLDP — no
//! port-number translation is needed.
//!
//! Although CDP is Cisco-specific it is a standard discovery method here
//! (alongside `lldp`), so it runs for every device; non-Cisco devices simply
//! return an empty cache.

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use super::{apply_suffix_column, port_mut, MibSupport, Neighbor, NeighborProtocol, NetworkDevice};
use crate::snmp::query::SnmpQuery;

// cdpCacheTable columns (1.3.6.1.4.1.9.9.23.1.2.1.1.N).
const CDP_CACHE: [u64; 13] = [1, 3, 6, 1, 4, 1, 9, 9, 23, 1, 2, 1, 1];
const CDP_VERSION: u64 = 5;
const CDP_DEVICE_ID: u64 = 6;
const CDP_DEVICE_PORT: u64 = 7;

/// MIB module for CDP neighbor discovery.
#[derive(Debug, Default, Clone, Copy)]
pub struct CdpMib;

#[async_trait]
impl MibSupport for CdpMib {
    fn name(&self) -> &'static str {
        "cdp"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let mut neighbors: BTreeMap<Vec<u64>, Neighbor> = BTreeMap::new();
        let new = |_: &[u64]| Neighbor::new(NeighborProtocol::Cdp);

        apply_suffix_column(
            session,
            &column(CDP_DEVICE_ID),
            &mut neighbors,
            new,
            |n, v| {
                n.sys_name = v.as_str();
            },
        )
        .await?;
        apply_suffix_column(
            session,
            &column(CDP_VERSION),
            &mut neighbors,
            new,
            |n, v| {
                n.sys_descr = v.as_str();
            },
        )
        .await?;
        apply_suffix_column(
            session,
            &column(CDP_DEVICE_PORT),
            &mut neighbors,
            new,
            |n, v| {
                n.port_id = v.as_str();
            },
        )
        .await?;

        for (suffix, neighbor) in neighbors {
            // suffix = [cdpCacheIfIndex, cdpCacheDeviceIndex]; the first is ifIndex.
            if let (false, Some(&ifindex)) = (neighbor.is_empty(), suffix.first()) {
                port_mut(device, ifindex).neighbors.push(neighbor);
            }
        }
        device.ports.sort_by_key(|p| p.index);
        Ok(())
    }
}

/// Builds the OID of `cdpCacheTable` column `col`.
fn column(col: u64) -> Vec<u64> {
    let mut oid = CDP_CACHE.to_vec();
    oid.push(col);
    oid
}

#[cfg(test)]
mod tests {
    use super::CdpMib;
    use crate::snmp::mib::{MibSupport, NeighborProtocol, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    // One CDP neighbor on local ifIndex 5.
    const CDP_WALK: &str = r#".1.3.6.1.4.1.9.9.23.1.2.1.1.5.5.1 = STRING: "Cisco IOS Software 15.0"
.1.3.6.1.4.1.9.9.23.1.2.1.1.6.5.1 = STRING: "neighbor-rtr.example.com"
.1.3.6.1.4.1.9.9.23.1.2.1.1.7.5.1 = STRING: "GigabitEthernet0/2"
"#;

    #[tokio::test]
    async fn attaches_cdp_neighbor_to_local_ifindex() {
        let mut session = WalkSession::parse(CDP_WALK).unwrap();
        let mut device = NetworkDevice::default();
        CdpMib.run(&mut session, &mut device).await.unwrap();

        assert_eq!(device.ports.len(), 1);
        let port = &device.ports[0];
        assert_eq!(port.index, 5);
        assert_eq!(port.neighbors.len(), 1);

        let n = &port.neighbors[0];
        assert_eq!(n.protocol, NeighborProtocol::Cdp);
        assert_eq!(n.sys_name.as_deref(), Some("neighbor-rtr.example.com"));
        assert_eq!(n.sys_descr.as_deref(), Some("Cisco IOS Software 15.0"));
        assert_eq!(n.port_id.as_deref(), Some("GigabitEthernet0/2"));
    }

    #[tokio::test]
    async fn no_cdp_cache_adds_no_ports() {
        let mut session = WalkSession::parse("").unwrap();
        let mut device = NetworkDevice::default();
        CdpMib.run(&mut session, &mut device).await.unwrap();
        assert!(device.ports.is_empty());
    }
}
