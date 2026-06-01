// SPDX-License-Identifier: GPL-2.0-only

//! Standard `IP-MIB` address support (the `ipAddrTable` fallback).
//!
//! Walks `ipAdEntIfIndex` to assign each configured IPv4 address to its
//! interface, populating [`Port`]'s `ips`. The address is the table index
//! (four arcs) and the value is the `ifIndex`. Loopback and unspecified
//! addresses are skipped. This is the per-port IP fallback the upstream agent
//! uses when richer per-port IP data is unavailable.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use async_trait::async_trait;
use glpi_core::error::Result;

use super::{as_number, instance_suffix, port_mut, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// `ipAdEntIfIndex` (1.3.6.1.2.1.4.20.1.2), indexed by the IPv4 address.
const IP_AD_ENT_IFINDEX: [u64; 10] = [1, 3, 6, 1, 2, 1, 4, 20, 1, 2];

/// MIB module for the `ipAddrTable` interface-address mapping.
#[derive(Debug, Default, Clone, Copy)]
pub struct IpMib;

#[async_trait]
impl MibSupport for IpMib {
    fn name(&self) -> &'static str {
        "ip"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let mut ips_by_ifindex: BTreeMap<u64, Vec<IpAddr>> = BTreeMap::new();

        for (oid, value) in session.walk(&IP_AD_ENT_IFINDEX).await? {
            let Some(suffix) = instance_suffix(&oid, &IP_AD_ENT_IFINDEX) else {
                continue;
            };
            let (Some(ip), Some(ifindex)) = (
                ipv4_from_arcs(&suffix),
                as_number(&value).and_then(|n| u64::try_from(n).ok()),
            ) else {
                continue;
            };
            if ip.is_loopback() || ip.is_unspecified() {
                continue;
            }
            ips_by_ifindex
                .entry(ifindex)
                .or_default()
                .push(IpAddr::V4(ip));
        }

        for (ifindex, mut ips) in ips_by_ifindex {
            ips.sort_unstable();
            ips.dedup();
            port_mut(device, ifindex).ips = ips;
        }
        device.ports.sort_by_key(|p| p.index);
        Ok(())
    }
}

/// Converts the four-arc `ipAddrTable` index into an IPv4 address.
fn ipv4_from_arcs(arcs: &[u64]) -> Option<Ipv4Addr> {
    let octets: [u8; 4] = arcs
        .iter()
        .map(|a| u8::try_from(*a).ok())
        .collect::<Option<Vec<u8>>>()?
        .try_into()
        .ok()?;
    Some(Ipv4Addr::from(octets))
}

#[cfg(test)]
mod tests {
    use super::IpMib;
    use crate::snmp::mib::{MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;
    use std::net::{IpAddr, Ipv4Addr};

    // Two IPs on ifIndex 5; a loopback on ifIndex 1 that must be skipped.
    const IP_WALK: &str = r#".1.3.6.1.2.1.4.20.1.2.10.0.0.1 = INTEGER: 5
.1.3.6.1.2.1.4.20.1.2.192.168.1.1 = INTEGER: 5
.1.3.6.1.2.1.4.20.1.2.127.0.0.1 = INTEGER: 1
"#;

    #[tokio::test]
    async fn assigns_ips_to_interfaces_skipping_loopback() {
        let mut session = WalkSession::parse(IP_WALK).unwrap();
        let mut device = NetworkDevice::default();
        IpMib.run(&mut session, &mut device).await.unwrap();

        // Only ifIndex 5 gets a port; the loopback-only ifIndex 1 is skipped.
        assert_eq!(device.ports.len(), 1);
        let port = &device.ports[0];
        assert_eq!(port.index, 5);
        assert_eq!(
            port.ips,
            vec![
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            ]
        );
    }

    #[tokio::test]
    async fn no_ip_table_adds_no_ports() {
        let mut session = WalkSession::parse("").unwrap();
        let mut device = NetworkDevice::default();
        IpMib.run(&mut session, &mut device).await.unwrap();
        assert!(device.ports.is_empty());
    }
}
