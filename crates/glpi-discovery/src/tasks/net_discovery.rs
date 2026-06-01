// SPDX-License-Identifier: GPL-2.0-only

//! The NetDiscovery task.
//!
//! Scans one or more IPv4 ranges and, for every address, runs the liveness
//! methods (ping / ARP / NetBIOS) and an SNMP probe with each configured
//! credential. Any address that responds becomes a [`DiscoveredDevice`]
//! carrying its MAC, resolved name and — when SNMP answers — a [`SnmpDevice`]
//! with the system group plus the manufacturer/type/model classified from
//! `sysobject.ids`.
//!
//! The SNMP-enrichment core ([`discover_snmp`]) and the record merge
//! ([`DiscoveredDevice::from_parts`]) are pure/mockable and unit-tested; the
//! [`NetDiscoveryTask::run`] orchestration (bounded-concurrency scan over the
//! network) is exercised end-to-end against live targets.
//!
//! The IEC 61850 merge (plan Phase 4) hooks in here later, alongside the SNMP
//! device record.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use glpi_core::error::Result;
use glpi_core::types::network::MacAddress;
use glpi_core::types::snmp::SnmpCredentials;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::ip_range::Ipv4Range;
use crate::methods::{ArpMethod, NetBiosMethod, PingMethod};
use crate::snmp::client::{SnmpClient, SNMP_PORT};
use crate::snmp::query::{identify, SnmpQuery};
use crate::snmp::sysobject::SysObjectIds;
use crate::traits::{probe_address, DiscoveredHost, DiscoveryMethod};

/// `sysContact.0` — the contact person for the managed node.
pub const SYS_CONTACT: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 4, 0];
/// `sysLocation.0` — the physical location of the node.
pub const SYS_LOCATION: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 6, 0];

/// SNMP-derived view of a discovered device.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnmpDevice {
    /// `sysDescr` text.
    pub description: String,
    /// `sysName`, if set.
    pub name: Option<String>,
    /// `sysContact`, if set.
    pub contact: Option<String>,
    /// `sysLocation`, if set.
    pub location: Option<String>,
    /// `sysObjectID` in dotted form, if returned.
    pub sys_object_id: Option<String>,
    /// Manufacturer from `sysobject.ids`, when classified.
    pub manufacturer: Option<String>,
    /// Device type from `sysobject.ids`, when classified.
    pub r#type: Option<String>,
    /// Model from `sysobject.ids`, when classified.
    pub model: Option<String>,
}

/// A device found during a NetDiscovery scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    /// The address that responded.
    pub address: IpAddr,
    /// MAC address, when a method (ARP) resolved one.
    pub mac: Option<MacAddress>,
    /// Best resolved name: the DNS/NetBIOS hostname, else the SNMP `sysName`.
    pub name: Option<String>,
    /// SNMP details, when the host answered SNMP.
    pub snmp: Option<SnmpDevice>,
    /// Names of the methods that detected the device.
    pub found_by: Vec<&'static str>,
}

impl DiscoveredDevice {
    /// Merges the liveness result and the SNMP result for `address`.
    ///
    /// Returns `None` when neither produced a hit. The DNS/NetBIOS name takes
    /// precedence over the SNMP `sysName`; the SNMP probe contributes a
    /// `"snmp"` entry to [`found_by`](Self::found_by).
    #[must_use]
    pub fn from_parts(
        address: IpAddr,
        host: Option<DiscoveredHost>,
        snmp: Option<SnmpDevice>,
    ) -> Option<Self> {
        if host.is_none() && snmp.is_none() {
            return None;
        }
        let mut found_by = Vec::new();
        let mut mac = None;
        let mut name = None;
        if let Some(host) = host {
            found_by.extend(host.found_by);
            mac = host.mac;
            name = host.hostname;
        }
        if snmp.is_some() {
            found_by.push("snmp");
        }
        if name.is_none() {
            name = snmp.as_ref().and_then(|snmp| snmp.name.clone());
        }
        Some(Self {
            address,
            mac,
            name,
            snmp,
            found_by,
        })
    }
}

/// Reads the SNMP system group and classifies the device via `sysobjects`.
///
/// Returns `Ok(None)` if the host is not an SNMP node (no `sysDescr`).
///
/// # Errors
///
/// Propagates transport/protocol failures from `session`.
pub async fn discover_snmp(
    session: &mut dyn SnmpQuery,
    sysobjects: &SysObjectIds,
) -> Result<Option<SnmpDevice>> {
    let Some(info) = identify(session).await? else {
        return Ok(None);
    };

    let contact = get_string(session, &SYS_CONTACT).await?;
    let location = get_string(session, &SYS_LOCATION).await?;

    let classified = info
        .sys_object_id
        .as_deref()
        .and_then(|oid| sysobjects.lookup(oid));

    Ok(Some(SnmpDevice {
        description: info.sys_descr,
        name: info.sys_name,
        contact,
        location,
        sys_object_id: info.sys_object_id,
        manufacturer: classified.and_then(|c| c.manufacturer.clone()),
        r#type: classified.and_then(|c| c.r#type.clone()),
        model: classified.and_then(|c| c.model.clone()),
    }))
}

/// GETs `oid` and returns its non-empty string value, if any.
async fn get_string(session: &mut dyn SnmpQuery, oid: &[u64]) -> Result<Option<String>> {
    Ok(session
        .get(oid)
        .await?
        .and_then(|value| value.as_str())
        .filter(|s| !s.is_empty()))
}

/// Configuration and driver for a NetDiscovery scan.
///
/// Build with [`NetDiscoveryTask::new`] and the `with_*` setters, then
/// [`run`](Self::run).
#[derive(Clone)]
pub struct NetDiscoveryTask {
    ranges: Vec<Ipv4Range>,
    credentials: Arc<[SnmpCredentials]>,
    sysobjects: Arc<SysObjectIds>,
    concurrency: usize,
    timeout: Duration,
    snmp_retries: u32,
    snmp_port: u16,
    use_ping: bool,
    use_arp: bool,
    use_netbios: bool,
}

impl NetDiscoveryTask {
    /// Creates a task over `ranges` with defaults: ping + NetBIOS liveness
    /// (ARP off, as it needs the local cache), no SNMP credentials, a 1-second
    /// per-probe timeout and 64-way concurrency.
    #[must_use]
    pub fn new(ranges: Vec<Ipv4Range>) -> Self {
        Self {
            ranges,
            credentials: Arc::from(Vec::new()),
            sysobjects: Arc::new(SysObjectIds::default()),
            concurrency: 64,
            timeout: Duration::from_secs(1),
            snmp_retries: 0,
            snmp_port: SNMP_PORT,
            use_ping: true,
            use_arp: false,
            use_netbios: true,
        }
    }

    /// Sets the SNMP credentials tried against each address.
    #[must_use]
    pub fn with_credentials(mut self, credentials: Vec<SnmpCredentials>) -> Self {
        self.credentials = Arc::from(credentials);
        self
    }

    /// Sets the `sysobject.ids` database used to classify SNMP devices.
    #[must_use]
    pub fn with_sysobjects(mut self, sysobjects: SysObjectIds) -> Self {
        self.sysobjects = Arc::new(sysobjects);
        self
    }

    /// Sets the maximum number of addresses probed concurrently (min 1).
    #[must_use]
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// Sets the per-probe timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the SNMP retry count (the `snmp-retries` option).
    #[must_use]
    pub fn with_snmp_retries(mut self, retries: u32) -> Self {
        self.snmp_retries = retries;
        self
    }

    /// Enables or disables ARP-cache MAC resolution.
    #[must_use]
    pub fn with_arp(mut self, enabled: bool) -> Self {
        self.use_arp = enabled;
        self
    }

    /// Total number of addresses the scan will cover.
    #[must_use]
    pub fn target_count(&self) -> u64 {
        self.ranges.iter().map(Ipv4Range::len).sum()
    }

    /// Iterator over every target address.
    fn targets(&self) -> impl Iterator<Item = IpAddr> + '_ {
        self.ranges.iter().flat_map(Ipv4Range::iter).map(IpAddr::V4)
    }

    /// Builds the liveness methods enabled by the configuration.
    fn liveness_methods(&self) -> Vec<Arc<dyn DiscoveryMethod>> {
        let mut methods: Vec<Arc<dyn DiscoveryMethod>> = Vec::new();
        if self.use_ping {
            methods.push(Arc::new(PingMethod::new(self.timeout)));
        }
        if self.use_netbios {
            methods.push(Arc::new(NetBiosMethod::new(self.timeout)));
        }
        if self.use_arp {
            match ArpMethod::from_system() {
                Ok(arp) => methods.push(Arc::new(arp)),
                Err(err) => tracing::warn!(error = %err, "ARP method disabled: cannot read cache"),
            }
        }
        methods
    }

    /// Runs the scan, returning the discovered devices sorted by address.
    pub async fn run(&self) -> Vec<DiscoveredDevice> {
        let methods: Arc<[Arc<dyn DiscoveryMethod>]> = Arc::from(self.liveness_methods());
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let mut tasks: JoinSet<Option<DiscoveredDevice>> = JoinSet::new();

        for target in self.targets() {
            let methods = Arc::clone(&methods);
            let credentials = Arc::clone(&self.credentials);
            let sysobjects = Arc::clone(&self.sysobjects);
            let semaphore = Arc::clone(&semaphore);
            let (port, timeout, retries) = (self.snmp_port, self.timeout, self.snmp_retries);
            tasks.spawn(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .expect("scan semaphore is never closed");
                discover_one(
                    target,
                    &methods,
                    &credentials,
                    &sysobjects,
                    port,
                    timeout,
                    retries,
                )
                .await
            });
        }

        let mut devices = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(Some(device)) => devices.push(device),
                Ok(None) => {}
                Err(err) => tracing::warn!(error = %err, "discovery task panicked"),
            }
        }
        devices.sort_by_key(|device| match device.address {
            IpAddr::V4(v4) => (0u8, u128::from(u32::from(v4))),
            IpAddr::V6(v6) => (1u8, u128::from(v6)),
        });
        devices
    }
}

/// Probes one address: liveness methods plus an SNMP probe per credential.
async fn discover_one(
    target: IpAddr,
    methods: &[Arc<dyn DiscoveryMethod>],
    credentials: &[SnmpCredentials],
    sysobjects: &SysObjectIds,
    port: u16,
    timeout: Duration,
    retries: u32,
) -> Option<DiscoveredDevice> {
    let host = probe_address(target, methods, timeout).await;
    let snmp = discover_snmp_target(target, credentials, sysobjects, port, timeout, retries).await;
    DiscoveredDevice::from_parts(target, host, snmp)
}

/// Tries each credential against `target`, returning the first SNMP hit.
async fn discover_snmp_target(
    target: IpAddr,
    credentials: &[SnmpCredentials],
    sysobjects: &SysObjectIds,
    port: u16,
    timeout: Duration,
    retries: u32,
) -> Option<SnmpDevice> {
    for credential in credentials {
        let Ok(mut client) = SnmpClient::connect(target, port, credential, timeout, retries).await
        else {
            continue;
        };
        match discover_snmp(&mut client, sysobjects).await {
            Ok(Some(device)) => return Some(device),
            Ok(None) => {}
            Err(err) => tracing::debug!(%target, error = %err, "SNMP discovery failed"),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{discover_snmp, DiscoveredDevice, SnmpDevice, SYS_CONTACT, SYS_LOCATION};
    use crate::snmp::query::{SnmpQuery, SYS_DESCR, SYS_NAME, SYS_OBJECT_ID};
    use crate::snmp::sysobject::SysObjectIds;
    use crate::snmp::value::SnmpValue;
    use crate::traits::DiscoveredHost;
    use async_trait::async_trait;
    use glpi_core::error::Result;
    use glpi_core::types::network::MacAddress;
    use std::collections::BTreeMap;
    use std::net::{IpAddr, Ipv4Addr};

    #[derive(Default)]
    struct MapSession {
        entries: BTreeMap<Vec<u64>, SnmpValue>,
    }

    impl MapSession {
        fn with(mut self, oid: &[u64], value: SnmpValue) -> Self {
            self.entries.insert(oid.to_vec(), value);
            self
        }
    }

    #[async_trait]
    impl SnmpQuery for MapSession {
        async fn get(&mut self, oid: &[u64]) -> Result<Option<SnmpValue>> {
            Ok(self.entries.get(oid).cloned())
        }
        async fn get_next(&mut self, _oid: &[u64]) -> Result<Option<(Vec<u64>, SnmpValue)>> {
            Ok(None)
        }
        async fn walk(&mut self, _root: &[u64]) -> Result<Vec<(Vec<u64>, SnmpValue)>> {
            Ok(Vec::new())
        }
    }

    fn sysobjects() -> SysObjectIds {
        SysObjectIds::parse("9.1.3\tCisco\tNETWORKING\tRouter xGS\n")
    }

    fn ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))
    }

    #[tokio::test]
    async fn discover_snmp_classifies_via_sysobject() {
        let mut session = MapSession::default()
            .with(&SYS_DESCR, SnmpValue::OctetString(b"Cisco IOS".to_vec()))
            .with(
                &SYS_OBJECT_ID,
                SnmpValue::Oid("1.3.6.1.4.1.9.1.3".to_owned()),
            )
            .with(&SYS_NAME, SnmpValue::OctetString(b"core-sw".to_vec()))
            .with(&SYS_CONTACT, SnmpValue::OctetString(b"netops".to_vec()))
            .with(&SYS_LOCATION, SnmpValue::OctetString(b"rack 1".to_vec()));

        let device = discover_snmp(&mut session, &sysobjects())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(device.description, "Cisco IOS");
        assert_eq!(device.name.as_deref(), Some("core-sw"));
        assert_eq!(device.contact.as_deref(), Some("netops"));
        assert_eq!(device.location.as_deref(), Some("rack 1"));
        assert_eq!(device.manufacturer.as_deref(), Some("Cisco"));
        assert_eq!(device.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.model.as_deref(), Some("Router xGS"));
    }

    #[tokio::test]
    async fn discover_snmp_none_when_not_snmp() {
        let mut session = MapSession::default();
        assert!(discover_snmp(&mut session, &sysobjects())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn discover_snmp_unclassified_when_oid_unknown() {
        let mut session = MapSession::default()
            .with(&SYS_DESCR, SnmpValue::OctetString(b"Mystery".to_vec()))
            .with(
                &SYS_OBJECT_ID,
                SnmpValue::Oid("1.3.6.1.4.1.99999.1".to_owned()),
            );
        let device = discover_snmp(&mut session, &sysobjects())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(device.description, "Mystery");
        assert_eq!(device.manufacturer, None);
        assert_eq!(device.r#type, None);
    }

    #[test]
    fn from_parts_returns_none_when_nothing_responded() {
        assert!(DiscoveredDevice::from_parts(ip(), None, None).is_none());
    }

    #[test]
    fn from_parts_merges_liveness_and_snmp() {
        let host = DiscoveredHost {
            address: ip(),
            mac: Some(MacAddress::new([1, 2, 3, 4, 5, 6])),
            hostname: Some("nb-name".to_owned()),
            found_by: vec!["ping", "netbios"],
        };
        let snmp = SnmpDevice {
            description: "dev".to_owned(),
            name: Some("snmp-name".to_owned()),
            ..SnmpDevice::default()
        };
        let device = DiscoveredDevice::from_parts(ip(), Some(host), Some(snmp)).unwrap();
        assert_eq!(device.found_by, vec!["ping", "netbios", "snmp"]);
        assert_eq!(device.mac, Some(MacAddress::new([1, 2, 3, 4, 5, 6])));
        // DNS/NetBIOS name wins over the SNMP sysName.
        assert_eq!(device.name.as_deref(), Some("nb-name"));
    }

    #[test]
    fn from_parts_uses_snmp_name_without_a_hostname() {
        let snmp = SnmpDevice {
            description: "dev".to_owned(),
            name: Some("snmp-name".to_owned()),
            ..SnmpDevice::default()
        };
        let device = DiscoveredDevice::from_parts(ip(), None, Some(snmp)).unwrap();
        assert_eq!(device.found_by, vec!["snmp"]);
        assert_eq!(device.name.as_deref(), Some("snmp-name"));
    }
}
