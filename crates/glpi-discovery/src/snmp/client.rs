// SPDX-License-Identifier: GPL-2.0-only

//! Async SNMP client wrapping [`snmp2::AsyncSession`].
//!
//! [`SnmpClient`] adds the three things the bare session lacks for our use:
//!
//! * **timeout + retries** — `snmp2`'s send/receive has no timeout of its own,
//!   so each request is bounded by a deadline and retried up to `snmp-retries`
//!   times (default 0 → a single attempt);
//! * **owned results** — responses borrow the session's receive buffer, so each
//!   varbind is projected into an owned `(Vec<u64>, SnmpValue)` before return;
//! * **`walk`** — built on repeated `getnext`, stopping at the end of the
//!   subtree, an `endOfMibView`, or any non-advancing response.
//!
//! OIDs are passed and returned as numeric arc vectors (`&[u64]` /
//! `Vec<u64>`), which suit MIB-table index extraction.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use glpi_core::error::{AgentError, Result};
use glpi_core::types::snmp::{SnmpCredentials, SnmpVersion};
use snmp2::{AsyncSession, Oid, Pdu};

use super::credentials::{build_security, community};
use super::value::SnmpValue;

/// Default UDP port of the SNMP agent.
pub const SNMP_PORT: u16 = 161;

/// An async SNMP session against one target, with timeout/retry handling.
pub struct SnmpClient {
    session: AsyncSession,
    timeout: Duration,
    retries: u32,
}

impl SnmpClient {
    /// Opens a session to `target:port` using `creds`.
    ///
    /// For v3 the USM engine-discovery handshake ([`AsyncSession::init`]) runs
    /// under `timeout`. `retries` is the number of *additional* attempts after
    /// the first (so `0` means try once).
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials are invalid for the version, the
    /// socket cannot be created, or (v3) the engine-discovery handshake fails.
    pub async fn connect(
        target: IpAddr,
        port: u16,
        creds: &SnmpCredentials,
        timeout: Duration,
        retries: u32,
    ) -> Result<Self> {
        let dest = SocketAddr::new(target, port);
        let req_id = 0;

        // The session constructors fail with std::io::Error (socket setup);
        // get/getnext/init fail with snmp2::Error.
        let session = match creds.version {
            SnmpVersion::V1 => AsyncSession::new_v1(dest, community(creds)?, req_id).await?,
            SnmpVersion::V2c => AsyncSession::new_v2c(dest, community(creds)?, req_id).await?,
            SnmpVersion::V3 => {
                let security = build_security(creds)?;
                let mut session = AsyncSession::new_v3(dest, req_id, security).await?;
                match tokio::time::timeout(timeout, session.init()).await {
                    Ok(result) => result.map_err(map_snmp_err)?,
                    Err(_) => {
                        return Err(AgentError::Transport(format!(
                            "SNMPv3 engine discovery timed out for {target}"
                        )))
                    }
                }
                session
            }
        };

        Ok(Self {
            session,
            timeout,
            retries,
        })
    }

    /// Performs a GET for a single OID, returning its value (including any
    /// SNMPv2 exception value such as `noSuchObject`).
    ///
    /// # Errors
    ///
    /// Returns an error if every attempt times out or the agent reports a
    /// transport/protocol failure.
    pub async fn get(&mut self, oid: &[u64]) -> Result<Option<SnmpValue>> {
        let oid = make_oid(oid)?;
        let mut attempt = 0;
        loop {
            match tokio::time::timeout(self.timeout, self.session.get(&oid)).await {
                Ok(Ok(pdu)) => return Ok(first_varbind(&pdu).map(|(_, value)| value)),
                Ok(Err(err)) if attempt >= self.retries => return Err(map_snmp_err(err)),
                Err(_) if attempt >= self.retries => {
                    return Err(AgentError::Transport("SNMP GET timed out".to_owned()))
                }
                _ => attempt += 1,
            }
        }
    }

    /// Performs a GETNEXT, returning the next OID (as arcs) and its value.
    ///
    /// # Errors
    ///
    /// As for [`SnmpClient::get`].
    pub async fn get_next(&mut self, oid: &[u64]) -> Result<Option<(Vec<u64>, SnmpValue)>> {
        let oid = make_oid(oid)?;
        self.query_next(&oid).await
    }

    /// Walks the subtree rooted at `root`, returning every `(oid, value)` pair
    /// beneath it in lexicographic order.
    ///
    /// The walk stops at the first response that leaves the subtree, signals
    /// `endOfMibView`, or fails to advance past the previous OID (a guard
    /// against a misbehaving agent looping the walk).
    ///
    /// # Errors
    ///
    /// As for [`SnmpClient::get`].
    pub async fn walk(&mut self, root: &[u64]) -> Result<Vec<(Vec<u64>, SnmpValue)>> {
        let mut results = Vec::new();
        let mut current = root.to_vec();
        loop {
            let oid = make_oid(&current)?;
            let Some((next, value)) = self.query_next(&oid).await? else {
                break;
            };
            if value.is_exception() || !next.starts_with(root) || next <= current {
                break;
            }
            current = next.clone();
            results.push((next, value));
        }
        Ok(results)
    }

    /// GETNEXT with timeout/retry, projecting the first varbind to owned form.
    async fn query_next(&mut self, oid: &Oid<'_>) -> Result<Option<(Vec<u64>, SnmpValue)>> {
        let mut attempt = 0;
        loop {
            match tokio::time::timeout(self.timeout, self.session.getnext(oid)).await {
                Ok(Ok(pdu)) => {
                    return match first_varbind(&pdu) {
                        Some((oid_str, value)) => Ok(Some((parse_oid_parts(&oid_str)?, value))),
                        None => Ok(None),
                    }
                }
                Ok(Err(err)) if attempt >= self.retries => return Err(map_snmp_err(err)),
                Err(_) if attempt >= self.retries => {
                    return Err(AgentError::Transport("SNMP GETNEXT timed out".to_owned()))
                }
                _ => attempt += 1,
            }
        }
    }
}

/// Extracts the first varbind of a response as `(dotted-oid, value)`.
///
/// Returns `None` if the agent reported an error status (e.g. v1 `noSuchName`
/// at the end of the MIB) or the response carried no varbind.
fn first_varbind(pdu: &Pdu<'_>) -> Option<(String, SnmpValue)> {
    if pdu.error_status != 0 {
        return None;
    }
    pdu.varbinds
        .clone()
        .next()
        .and_then(|(oid, value)| SnmpValue::from_snmp2(&value).map(|v| (oid.to_string(), v)))
}

/// Builds an `snmp2` OID from numeric arcs.
fn make_oid(parts: &[u64]) -> Result<Oid<'static>> {
    Oid::from(parts).map_err(|e| AgentError::Parse(format!("invalid OID {parts:?}: {e:?}")))
}

/// Parses a dotted-decimal OID string into numeric arcs.
fn parse_oid_parts(dotted: &str) -> Result<Vec<u64>> {
    dotted
        .split('.')
        .map(|arc| {
            arc.parse::<u64>()
                .map_err(|_| AgentError::Parse(format!("invalid OID arc in {dotted:?}")))
        })
        .collect()
}

/// Maps an `snmp2` error onto the workspace error type.
fn map_snmp_err(err: snmp2::Error) -> AgentError {
    AgentError::Transport(format!("SNMP error: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{make_oid, parse_oid_parts};

    #[test]
    fn round_trips_oid_parts_through_dotted_string() {
        let parts = vec![1, 3, 6, 1, 2, 1, 1, 1, 0];
        let oid = make_oid(&parts).unwrap();
        assert_eq!(parse_oid_parts(&oid.to_string()).unwrap(), parts);
    }

    #[test]
    fn parses_dotted_oid() {
        assert_eq!(
            parse_oid_parts("1.3.6.1.2.1").unwrap(),
            vec![1, 3, 6, 1, 2, 1]
        );
    }

    #[test]
    fn rejects_malformed_dotted_oid() {
        assert!(parse_oid_parts("1.3.x.1").is_err());
        assert!(parse_oid_parts("").is_err());
    }

    #[test]
    fn subtree_prefix_logic_matches_walk_termination() {
        // The walk keeps going while the next OID stays under the root and
        // advances; these are the exact predicates walk() relies on.
        let root = [1u64, 3, 6, 1, 2, 1, 1];
        let inside = [1u64, 3, 6, 1, 2, 1, 1, 1, 0];
        let outside = [1u64, 3, 6, 1, 2, 1, 2, 1, 0];
        assert!(inside.starts_with(&root));
        assert!(!outside.starts_with(&root));
        assert!(inside.to_vec() > root.to_vec());
    }
}
