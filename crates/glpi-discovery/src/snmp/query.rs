// SPDX-License-Identifier: GPL-2.0-only

//! The [`SnmpQuery`] abstraction and host identification.
//!
//! [`SnmpClient`] talks to a real agent, but the logic that *interprets* SNMP
//! data — host identification here, the MIB modules in Phase 3 — should be
//! testable without a network. [`SnmpQuery`] is that seam: an object-safe trait
//! covering `get` / `get_next` / `walk`, implemented by [`SnmpClient`] for
//! production and by a map-backed fake in tests (mirroring the OID→value shape
//! of the upstream `resources/walks/*` fixtures).
//!
//! [`identify`] reads the standard system group to decide whether a host speaks
//! SNMP and to recover its description, object identifier and name.

use async_trait::async_trait;
use glpi_core::error::Result;

use super::client::SnmpClient;
use super::value::SnmpValue;

/// `sysDescr.0` — a textual description of the entity.
pub const SYS_DESCR: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 1, 0];
/// `sysObjectID.0` — the vendor's authoritative identification of the device.
pub const SYS_OBJECT_ID: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 2, 0];
/// `sysName.0` — the administratively assigned node name.
pub const SYS_NAME: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 5, 0];

/// An object-safe SNMP query interface.
///
/// Implemented by [`SnmpClient`] over the wire and by test fakes over a static
/// OID map. All methods take `&mut self` because the underlying session is
/// stateful (request ids, v3 engine counters).
#[async_trait]
pub trait SnmpQuery: Send {
    /// GETs a single OID's value.
    ///
    /// # Errors
    ///
    /// Propagates transport/protocol failures from the underlying session.
    async fn get(&mut self, oid: &[u64]) -> Result<Option<SnmpValue>>;

    /// GETNEXTs from `oid`, returning the next OID and its value.
    ///
    /// # Errors
    ///
    /// Propagates transport/protocol failures from the underlying session.
    async fn get_next(&mut self, oid: &[u64]) -> Result<Option<(Vec<u64>, SnmpValue)>>;

    /// Walks the subtree rooted at `root`.
    ///
    /// # Errors
    ///
    /// Propagates transport/protocol failures from the underlying session.
    async fn walk(&mut self, root: &[u64]) -> Result<Vec<(Vec<u64>, SnmpValue)>>;
}

#[async_trait]
impl SnmpQuery for SnmpClient {
    async fn get(&mut self, oid: &[u64]) -> Result<Option<SnmpValue>> {
        // Inherent methods take resolution priority, so this is not recursive.
        SnmpClient::get(self, oid).await
    }

    async fn get_next(&mut self, oid: &[u64]) -> Result<Option<(Vec<u64>, SnmpValue)>> {
        SnmpClient::get_next(self, oid).await
    }

    async fn walk(&mut self, root: &[u64]) -> Result<Vec<(Vec<u64>, SnmpValue)>> {
        SnmpClient::walk(self, root).await
    }
}

/// The standard system-group identity of an SNMP host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnmpSysInfo {
    /// `sysDescr` — free-form device description (often vendor/model/firmware).
    pub sys_descr: String,
    /// `sysObjectID` — the vendor OID used to classify the device.
    pub sys_object_id: Option<String>,
    /// `sysName` — the device's configured name, when set.
    pub sys_name: Option<String>,
}

/// Identifies a host by reading its system group.
///
/// Returns `Ok(None)` when `sysDescr` is absent or an SNMPv2 exception — i.e.
/// the host did not answer as an SNMP node. Otherwise returns the description
/// plus whatever `sysObjectID` / `sysName` it exposes.
///
/// # Errors
///
/// Propagates transport/protocol failures from `session`.
pub async fn identify(session: &mut dyn SnmpQuery) -> Result<Option<SnmpSysInfo>> {
    let Some(descr) = session
        .get(&SYS_DESCR)
        .await?
        .filter(|value| !value.is_exception())
    else {
        return Ok(None);
    };

    let sys_object_id = session
        .get(&SYS_OBJECT_ID)
        .await?
        .and_then(|value| match value {
            SnmpValue::Oid(oid) => Some(oid),
            _ => None,
        });

    let sys_name = session
        .get(&SYS_NAME)
        .await?
        .and_then(|value| value.as_str())
        .filter(|name| !name.is_empty());

    Ok(Some(SnmpSysInfo {
        sys_descr: descr.as_str().unwrap_or_default(),
        sys_object_id,
        sys_name,
    }))
}

#[cfg(test)]
mod tests {
    use super::{identify, SnmpQuery, SYS_DESCR, SYS_NAME, SYS_OBJECT_ID};
    use crate::snmp::value::SnmpValue;
    use async_trait::async_trait;
    use glpi_core::error::Result;
    use std::collections::BTreeMap;
    use std::ops::Bound::{Excluded, Unbounded};

    /// A fake session backed by a static OID→value map, standing in for the
    /// `.walk` fixtures the MIB tests will use.
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

        async fn get_next(&mut self, oid: &[u64]) -> Result<Option<(Vec<u64>, SnmpValue)>> {
            Ok(self
                .entries
                .range((Excluded(oid.to_vec()), Unbounded))
                .next()
                .map(|(k, v)| (k.clone(), v.clone())))
        }

        async fn walk(&mut self, root: &[u64]) -> Result<Vec<(Vec<u64>, SnmpValue)>> {
            Ok(self
                .entries
                .iter()
                .filter(|(k, _)| k.starts_with(root))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect())
        }
    }

    #[tokio::test]
    async fn identify_reads_full_system_group() {
        let mut session = MapSession::default()
            .with(&SYS_DESCR, SnmpValue::OctetString(b"Cisco IOS".to_vec()))
            .with(
                &SYS_OBJECT_ID,
                SnmpValue::Oid("1.3.6.1.4.1.9.1.1".to_owned()),
            )
            .with(&SYS_NAME, SnmpValue::OctetString(b"core-sw-1".to_vec()));

        let info = identify(&mut session).await.unwrap().unwrap();
        assert_eq!(info.sys_descr, "Cisco IOS");
        assert_eq!(info.sys_object_id.as_deref(), Some("1.3.6.1.4.1.9.1.1"));
        assert_eq!(info.sys_name.as_deref(), Some("core-sw-1"));
    }

    #[tokio::test]
    async fn identify_returns_none_without_sysdescr() {
        let mut session = MapSession::default();
        assert!(identify(&mut session).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn identify_treats_exception_sysdescr_as_no_host() {
        let mut session = MapSession::default().with(&SYS_DESCR, SnmpValue::NoSuchObject);
        assert!(identify(&mut session).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn identify_tolerates_missing_optional_fields() {
        let mut session =
            MapSession::default().with(&SYS_DESCR, SnmpValue::OctetString(b"printer".to_vec()));
        let info = identify(&mut session).await.unwrap().unwrap();
        assert_eq!(info.sys_descr, "printer");
        assert_eq!(info.sys_object_id, None);
        // An empty sysName must not be reported as a name.
        assert_eq!(info.sys_name, None);
    }
}
