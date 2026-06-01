// SPDX-License-Identifier: GPL-2.0-only

//! Standard `BRIDGE-MIB` forwarding-database support (RFC 4188).
//!
//! Reads the transparent-bridging forwarding database to learn which MAC
//! addresses appear on which interface, populating each [`Port`]'s
//! `connected_macs`. The mapping chains two tables:
//!
//! * `dot1dTpFdbPort` — MAC (the table index) → bridge port number;
//! * `dot1dBasePortIfIndex` — bridge port number → `ifIndex`.
//!
//! Only `learned(3)` entries are kept, so the bridge's own (`self`) addresses
//! and management entries are excluded. Runs after `if`, attaching to the
//! existing ports (and creating one only if the FDB references an unknown
//! `ifIndex`).

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use glpi_core::error::Result;
use glpi_core::types::network::MacAddress;

use super::{as_number, table_index, MibSupport, NetworkDevice, Port};
use crate::snmp::query::SnmpQuery;

const DOT1D_BASE_PORT_IFINDEX: [u64; 11] = [1, 3, 6, 1, 2, 1, 17, 1, 4, 1, 2];
const DOT1D_TP_FDB_PORT: [u64; 11] = [1, 3, 6, 1, 2, 1, 17, 4, 3, 1, 2];
const DOT1D_TP_FDB_STATUS: [u64; 11] = [1, 3, 6, 1, 2, 1, 17, 4, 3, 1, 3];

/// `dot1dTpFdbStatus` value for a dynamically learned address.
const FDB_STATUS_LEARNED: i64 = 3;

/// MIB module for the transparent-bridging forwarding database.
#[derive(Debug, Default, Clone, Copy)]
pub struct BridgeMib;

#[async_trait]
impl MibSupport for BridgeMib {
    fn name(&self) -> &'static str {
        "bridge"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // Bridge port number -> ifIndex.
        let mut base_to_ifindex: HashMap<u64, u64> = HashMap::new();
        for (oid, value) in session.walk(&DOT1D_BASE_PORT_IFINDEX).await? {
            if let (Some(base_port), Some(ifindex)) = (
                table_index(&oid, &DOT1D_BASE_PORT_IFINDEX),
                as_number(&value).and_then(|n| u64::try_from(n).ok()),
            ) {
                base_to_ifindex.insert(base_port, ifindex);
            }
        }

        // MAC -> forwarding status, to keep only learned entries.
        let mut status: HashMap<Vec<u64>, i64> = HashMap::new();
        for (oid, value) in session.walk(&DOT1D_TP_FDB_STATUS).await? {
            if let (Some(mac), Some(state)) =
                (fdb_index(&oid, &DOT1D_TP_FDB_STATUS), as_number(&value))
            {
                status.insert(mac, state);
            }
        }

        // MAC -> ifIndex via the forwarding database.
        let mut macs_by_ifindex: BTreeMap<u64, Vec<MacAddress>> = BTreeMap::new();
        for (oid, value) in session.walk(&DOT1D_TP_FDB_PORT).await? {
            let Some(index) = fdb_index(&oid, &DOT1D_TP_FDB_PORT) else {
                continue;
            };
            // Skip non-learned entries (self/mgmt); accept ones with no status.
            if status.get(&index).is_some_and(|s| *s != FDB_STATUS_LEARNED) {
                continue;
            }
            let (Some(mac), Some(base_port)) = (
                mac_from_arcs(&index),
                as_number(&value).and_then(|n| u64::try_from(n).ok()),
            ) else {
                continue;
            };
            if let Some(&ifindex) = base_to_ifindex.get(&base_port) {
                macs_by_ifindex.entry(ifindex).or_default().push(mac);
            }
        }

        merge_into_ports(device, macs_by_ifindex);
        Ok(())
    }
}

/// Returns the MAC-address index (six arcs) of `oid` under FDB column `base`.
fn fdb_index(oid: &[u64], base: &[u64]) -> Option<Vec<u64>> {
    (oid.len() == base.len() + 6 && oid.starts_with(base)).then(|| oid[base.len()..].to_vec())
}

/// Converts six OID arcs (each a byte) into a MAC address.
fn mac_from_arcs(arcs: &[u64]) -> Option<MacAddress> {
    let octets: [u8; 6] = arcs
        .iter()
        .map(|a| u8::try_from(*a).ok())
        .collect::<Option<Vec<u8>>>()?
        .try_into()
        .ok()?;
    Some(MacAddress::new(octets))
}

/// Attaches the learned MACs to existing ports by `ifIndex`, creating a port
/// only for an `ifIndex` the interface table did not report.
fn merge_into_ports(device: &mut NetworkDevice, macs_by_ifindex: BTreeMap<u64, Vec<MacAddress>>) {
    for (ifindex, mut macs) in macs_by_ifindex {
        macs.sort_by_key(MacAddress::octets);
        macs.dedup();
        if let Some(port) = device.ports.iter_mut().find(|p| p.index == ifindex) {
            port.connected_macs = macs;
        } else {
            let mut port = Port::new(ifindex);
            port.connected_macs = macs;
            device.ports.push(port);
        }
    }
    device.ports.sort_by_key(|p| p.index);
}

#[cfg(test)]
mod tests {
    use super::BridgeMib;
    use crate::snmp::mib::{MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;
    use glpi_core::types::network::MacAddress;

    // basePort 1 -> ifIndex 10, basePort 2 -> ifIndex 20.
    // FDB: aa:bb:cc:00:00:01 learned on port 1; ..02 learned on port 2;
    //      ..03 is self(4) and must be excluded.
    const BRIDGE_WALK: &str = r#".1.3.6.1.2.1.17.1.4.1.2.1 = INTEGER: 10
.1.3.6.1.2.1.17.1.4.1.2.2 = INTEGER: 20
.1.3.6.1.2.1.17.4.3.1.2.170.187.204.0.0.1 = INTEGER: 1
.1.3.6.1.2.1.17.4.3.1.2.170.187.204.0.0.2 = INTEGER: 2
.1.3.6.1.2.1.17.4.3.1.2.170.187.204.0.0.3 = INTEGER: 1
.1.3.6.1.2.1.17.4.3.1.3.170.187.204.0.0.1 = INTEGER: 3
.1.3.6.1.2.1.17.4.3.1.3.170.187.204.0.0.2 = INTEGER: 3
.1.3.6.1.2.1.17.4.3.1.3.170.187.204.0.0.3 = INTEGER: 4
"#;

    #[tokio::test]
    async fn maps_learned_macs_to_interfaces() {
        let mut session = WalkSession::parse(BRIDGE_WALK).unwrap();
        let mut device = NetworkDevice::default();
        BridgeMib.run(&mut session, &mut device).await.unwrap();

        assert_eq!(device.ports.len(), 2);
        let p10 = device.ports.iter().find(|p| p.index == 10).unwrap();
        assert_eq!(
            p10.connected_macs,
            vec![MacAddress::new([0xaa, 0xbb, 0xcc, 0, 0, 1])]
        );
        let p20 = device.ports.iter().find(|p| p.index == 20).unwrap();
        assert_eq!(
            p20.connected_macs,
            vec![MacAddress::new([0xaa, 0xbb, 0xcc, 0, 0, 2])]
        );
    }

    #[tokio::test]
    async fn no_bridge_data_leaves_ports_untouched() {
        let mut session = WalkSession::parse("").unwrap();
        let mut device = NetworkDevice::default();
        BridgeMib.run(&mut session, &mut device).await.unwrap();
        assert!(device.ports.is_empty());
    }
}
