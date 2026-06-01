// SPDX-License-Identifier: GPL-2.0-only

//! The NetInventory task.
//!
//! Where NetDiscovery finds and roughly classifies devices, NetInventory does
//! the deep SNMP inventory of a single target: it opens a session (trying each
//! configured credential), runs the [`MibRegistry`] against it, and returns the
//! assembled [`NetworkDevice`] (system info, ports, components, plus the
//! `sysobject.ids` classification the registry applies).
//!
//! The interpretation work lives in the registry and the MIB modules (all
//! unit-tested via `WalkSession`); this task is the thin network wrapper that
//! connects and selects a working credential.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use glpi_core::error::Result;
use glpi_core::types::snmp::SnmpCredentials;

use crate::snmp::client::{SnmpClient, SNMP_PORT};
use crate::snmp::mib::{MibRegistry, NetworkDevice};
use crate::snmp::sysobject::SysObjectIds;

/// Configuration and driver for inventorying a device over SNMP.
#[derive(Clone)]
pub struct NetInventoryTask {
    credentials: Arc<[SnmpCredentials]>,
    registry: MibRegistry,
    sysobjects: Arc<SysObjectIds>,
    port: u16,
    timeout: Duration,
    snmp_retries: u32,
}

impl NetInventoryTask {
    /// Creates a task that tries `credentials` in order, running all standard
    /// MIB modules. Defaults: UDP 161, 1-second per-request timeout, no retries,
    /// empty `sysobject.ids`.
    #[must_use]
    pub fn new(credentials: Vec<SnmpCredentials>) -> Self {
        Self {
            credentials: Arc::from(credentials),
            registry: MibRegistry::with_standard(),
            sysobjects: Arc::new(SysObjectIds::default()),
            port: SNMP_PORT,
            timeout: Duration::from_secs(1),
            snmp_retries: 0,
        }
    }

    /// Replaces the MIB registry (e.g. to add vendor modules).
    #[must_use]
    pub fn with_registry(mut self, registry: MibRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Sets the `sysobject.ids` classification database.
    #[must_use]
    pub fn with_sysobjects(mut self, sysobjects: SysObjectIds) -> Self {
        self.sysobjects = Arc::new(sysobjects);
        self
    }

    /// Overrides the UDP port (default [`SNMP_PORT`]).
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the per-request timeout.
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

    /// Inventories `target`, returning its [`NetworkDevice`].
    ///
    /// Tries each credential in turn and returns the first that yields a device
    /// answering SNMP (a `sysDescr` is present). Returns `Ok(None)` if no
    /// credential elicits a response.
    ///
    /// # Errors
    ///
    /// Never fails on a per-credential connect/MIB error (those are logged and
    /// the next credential is tried); reserved for future hard failures.
    pub async fn inventory(&self, target: IpAddr) -> Result<Option<NetworkDevice>> {
        for credential in self.credentials.iter() {
            let Ok(mut client) = SnmpClient::connect(
                target,
                self.port,
                credential,
                self.timeout,
                self.snmp_retries,
            )
            .await
            else {
                continue;
            };
            match self.registry.inventory(&mut client, &self.sysobjects).await {
                // A populated sysDescr means the device actually answered.
                Ok(device) if device.info.description.is_some() => return Ok(Some(device)),
                Ok(_) => {}
                Err(err) => {
                    tracing::debug!(%target, error = %err, "SNMP inventory failed; trying next credential");
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::NetInventoryTask;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn no_credentials_yields_no_device_without_touching_the_network() {
        let task = NetInventoryTask::new(Vec::new());
        let target = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)); // TEST-NET-1, never contacted
        assert_eq!(task.inventory(target).await.unwrap(), None);
    }
}
