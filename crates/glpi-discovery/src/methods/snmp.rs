// SPDX-License-Identifier: GPL-2.0-only

//! SNMP discovery method.
//!
//! Detects SNMP-capable hosts by opening a session with each configured
//! credential in turn and reading the system group via [`identify`]. The first
//! credential that yields a `sysDescr` wins; the host's `sysName` (when set)
//! becomes its [`hostname`](crate::traits::Probe::hostname). Connection
//! failures and timeouts for a credential are treated as "this credential did
//! not work" and the next is tried; if none respond the host is reported as not
//! found (rather than an error), matching the other discovery methods.
//!
//! The richer identity (`sysObjectID`, full `sysDescr`) that drives device
//! classification is consumed by the NetDiscovery/NetInventory tasks, which
//! call [`identify`] directly; as a scanner method this only contributes
//! liveness and a hostname.

use std::net::IpAddr;
use std::time::Duration;

use async_trait::async_trait;
use glpi_core::error::Result;
use glpi_core::types::snmp::SnmpCredentials;

use crate::snmp::client::{SnmpClient, SNMP_PORT};
use crate::snmp::query::identify;
use crate::traits::{DiscoveryMethod, Probe};

/// Discovery method that probes a host over SNMP with one or more credentials.
#[derive(Debug, Clone)]
pub struct SnmpMethod {
    credentials: Vec<SnmpCredentials>,
    port: u16,
    timeout: Duration,
    retries: u32,
}

impl SnmpMethod {
    /// Creates a method that tries each of `credentials` in order, bounding
    /// each request by `timeout`. Defaults to UDP port 161 and no retries.
    #[must_use]
    pub fn new(credentials: Vec<SnmpCredentials>, timeout: Duration) -> Self {
        Self {
            credentials,
            port: SNMP_PORT,
            timeout,
            retries: 0,
        }
    }

    /// Overrides the UDP port (default [`SNMP_PORT`]).
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the number of retries per request (the `snmp-retries` option).
    #[must_use]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}

#[async_trait]
impl DiscoveryMethod for SnmpMethod {
    fn name(&self) -> &'static str {
        "snmp"
    }

    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>> {
        for credentials in &self.credentials {
            let mut client = match SnmpClient::connect(
                target,
                self.port,
                credentials,
                self.timeout,
                self.retries,
            )
            .await
            {
                Ok(client) => client,
                Err(err) => {
                    tracing::debug!(%target, error = %err, "SNMP connect failed; trying next credential");
                    continue;
                }
            };

            match identify(&mut client).await {
                Ok(Some(info)) => {
                    return Ok(Some(Probe {
                        mac: None,
                        hostname: info.sys_name,
                    }))
                }
                Ok(None) => {} // responded but not an SNMP node for these creds
                Err(err) => {
                    tracing::debug!(%target, error = %err, "SNMP identify failed; trying next credential");
                }
            }
        }
        Ok(None)
    }
}
