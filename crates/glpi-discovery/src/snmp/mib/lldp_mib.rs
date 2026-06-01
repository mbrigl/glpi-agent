// SPDX-License-Identifier: GPL-2.0-only

//! Standard `LLDP-MIB` neighbor discovery (IEEE 802.1AB).
//!
//! Walks `lldpRemTable` to learn the neighbor seen on each local port — remote
//! system name/description, port id/description and chassis id — and attaches a
//! [`Neighbor`] to the corresponding [`Port`]. The table is indexed by
//! `(lldpRemTimeMark, lldpRemLocalPortNum, lldpRemIndex)`; the local port number
//! is matched against the device's `ifIndex` (as the upstream agent does).
//!
//! Chassis / port identifiers are formatted from their accompanying subtype: a
//! MAC-address subtype yields a `aa:bb:…` string (and the chassis MAC), other
//! subtypes are decoded as text.

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use glpi_core::error::Result;
use glpi_core::types::network::MacAddress;

use super::{
    apply_suffix_column, as_number, instance_suffix, port_mut, MibSupport, Neighbor,
    NeighborProtocol, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;
use crate::snmp::value::SnmpValue;

// lldpRemTable columns (1.0.8802.1.1.2.1.4.1.1.N).
const LLDP_REM: [u64; 10] = [1, 0, 8802, 1, 1, 2, 1, 4, 1, 1];
const REM_CHASSIS_SUBTYPE: u64 = 4;
const REM_CHASSIS_ID: u64 = 5;
const REM_PORT_SUBTYPE: u64 = 6;
const REM_PORT_ID: u64 = 7;
const REM_PORT_DESC: u64 = 8;
const REM_SYS_NAME: u64 = 9;
const REM_SYS_DESC: u64 = 10;

/// `LldpChassisIdSubtype` / `LldpPortIdSubtype` value for a MAC address.
const SUBTYPE_MAC_ADDRESS: i64 = 4;

/// MIB module for LLDP neighbor discovery.
#[derive(Debug, Default, Clone, Copy)]
pub struct LldpMib;

#[async_trait]
impl MibSupport for LldpMib {
    fn name(&self) -> &'static str {
        "lldp"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let mut neighbors: BTreeMap<Vec<u64>, Neighbor> = BTreeMap::new();
        let new = |_: &[u64]| Neighbor::new(NeighborProtocol::Lldp);

        apply_suffix_column(
            session,
            &column(REM_SYS_NAME),
            &mut neighbors,
            new,
            |n, v| {
                n.sys_name = v.as_str();
            },
        )
        .await?;
        apply_suffix_column(
            session,
            &column(REM_SYS_DESC),
            &mut neighbors,
            new,
            |n, v| {
                n.sys_descr = v.as_str();
            },
        )
        .await?;
        apply_suffix_column(
            session,
            &column(REM_PORT_DESC),
            &mut neighbors,
            new,
            |n, v| {
                n.port_descr = v.as_str();
            },
        )
        .await?;

        // Chassis id and port id need their subtype to be decoded.
        let chassis_subtypes = subtype_map(session, REM_CHASSIS_SUBTYPE).await?;
        decode_id_column(
            session,
            REM_CHASSIS_ID,
            &chassis_subtypes,
            &mut neighbors,
            |n, text, mac| {
                n.chassis_id = text;
                n.mac = mac;
            },
        )
        .await?;

        let port_subtypes = subtype_map(session, REM_PORT_SUBTYPE).await?;
        decode_id_column(
            session,
            REM_PORT_ID,
            &port_subtypes,
            &mut neighbors,
            |n, text, _mac| n.port_id = text,
        )
        .await?;

        for (suffix, neighbor) in neighbors {
            // suffix = [timeMark, localPortNum, remIndex]; attach to the local port.
            if let (false, Some(&local_port)) = (neighbor.is_empty(), suffix.get(1)) {
                port_mut(device, local_port).neighbors.push(neighbor);
            }
        }
        device.ports.sort_by_key(|p| p.index);
        Ok(())
    }
}

/// Builds the OID of `lldpRemTable` column `col`.
fn column(col: u64) -> Vec<u64> {
    let mut oid = LLDP_REM.to_vec();
    oid.push(col);
    oid
}

/// Walks a subtype column into a `suffix -> subtype` map.
async fn subtype_map(session: &mut dyn SnmpQuery, col: u64) -> Result<HashMap<Vec<u64>, i64>> {
    let base = column(col);
    let mut map = HashMap::new();
    for (oid, value) in session.walk(&base).await? {
        if let (Some(suffix), Some(subtype)) = (instance_suffix(&oid, &base), as_number(&value)) {
            map.insert(suffix, subtype);
        }
    }
    Ok(map)
}

/// Walks an id column, formats each value per its subtype, and applies `set`.
async fn decode_id_column<F>(
    session: &mut dyn SnmpQuery,
    col: u64,
    subtypes: &HashMap<Vec<u64>, i64>,
    neighbors: &mut BTreeMap<Vec<u64>, Neighbor>,
    set: F,
) -> Result<()>
where
    F: Fn(&mut Neighbor, Option<String>, Option<MacAddress>),
{
    let base = column(col);
    for (oid, value) in session.walk(&base).await? {
        let Some(suffix) = instance_suffix(&oid, &base) else {
            continue;
        };
        let (text, mac) = format_id(&value, subtypes.get(&suffix).copied());
        let neighbor = neighbors
            .entry(suffix)
            .or_insert_with(|| Neighbor::new(NeighborProtocol::Lldp));
        set(neighbor, text, mac);
    }
    Ok(())
}

/// Formats an LLDP id value: a MAC-address subtype becomes `aa:bb:…` (and the
/// MAC), anything else is decoded as text.
fn format_id(value: &SnmpValue, subtype: Option<i64>) -> (Option<String>, Option<MacAddress>) {
    if subtype == Some(SUBTYPE_MAC_ADDRESS) {
        if let SnmpValue::OctetString(bytes) = value {
            if let Ok(octets) = <[u8; 6]>::try_from(bytes.as_slice()) {
                let mac = MacAddress::new(octets);
                return (Some(mac.to_string()), Some(mac));
            }
        }
    }
    (value.as_str().filter(|s| !s.is_empty()), None)
}

#[cfg(test)]
mod tests {
    use super::LldpMib;
    use crate::snmp::mib::{MibSupport, NeighborProtocol, NetworkDevice};
    use crate::snmp::walk::WalkSession;
    use glpi_core::types::network::MacAddress;

    // One neighbor on local port 5: chassis MAC, port id "Gi0/1" (interfaceName
    // subtype, not a MAC), names and descriptions.
    const LLDP_WALK: &str = r#".1.0.8802.1.1.2.1.4.1.1.4.0.5.1 = INTEGER: 4
.1.0.8802.1.1.2.1.4.1.1.5.0.5.1 = Hex-STRING: 00 11 22 33 44 55
.1.0.8802.1.1.2.1.4.1.1.6.0.5.1 = INTEGER: 5
.1.0.8802.1.1.2.1.4.1.1.7.0.5.1 = STRING: "Gi0/1"
.1.0.8802.1.1.2.1.4.1.1.8.0.5.1 = STRING: "uplink"
.1.0.8802.1.1.2.1.4.1.1.9.0.5.1 = STRING: "neighbor-sw"
.1.0.8802.1.1.2.1.4.1.1.10.0.5.1 = STRING: "Neighbor IOS 15.2"
"#;

    #[tokio::test]
    async fn attaches_neighbor_to_local_port() {
        let mut session = WalkSession::parse(LLDP_WALK).unwrap();
        let mut device = NetworkDevice::default();
        LldpMib.run(&mut session, &mut device).await.unwrap();

        assert_eq!(device.ports.len(), 1);
        let port = &device.ports[0];
        assert_eq!(port.index, 5);
        assert_eq!(port.neighbors.len(), 1);

        let n = &port.neighbors[0];
        assert_eq!(n.protocol, NeighborProtocol::Lldp);
        assert_eq!(n.sys_name.as_deref(), Some("neighbor-sw"));
        assert_eq!(n.sys_descr.as_deref(), Some("Neighbor IOS 15.2"));
        assert_eq!(n.port_id.as_deref(), Some("Gi0/1"));
        assert_eq!(n.port_descr.as_deref(), Some("uplink"));
        assert_eq!(n.chassis_id.as_deref(), Some("00:11:22:33:44:55"));
        assert_eq!(
            n.mac,
            Some(MacAddress::new([0, 0x11, 0x22, 0x33, 0x44, 0x55]))
        );
    }

    #[tokio::test]
    async fn no_lldp_data_adds_no_ports() {
        let mut session = WalkSession::parse("").unwrap();
        let mut device = NetworkDevice::default();
        LldpMib.run(&mut session, &mut device).await.unwrap();
        assert!(device.ports.is_empty());
    }
}
