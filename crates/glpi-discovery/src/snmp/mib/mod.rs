// SPDX-License-Identifier: GPL-2.0-only

//! MIB-support framework for NetInventory.
//!
//! A [`MibSupport`] module reads part of a device over SNMP and contributes to
//! a [`NetworkDevice`]. The [`MibRegistry`] runs a set of modules in order and
//! classifies the device from `sysobject.ids` afterwards. Standard MIBs (the
//! system group, interfaces, …) always run; vendor MIBs (Phase 3, in batches)
//! will be selected by `sysObjectID` once they land.
//!
//! Modules are designed to compose: each only sets fields it can determine and
//! leaves the rest untouched, so a later module never clobbers an earlier
//! module's value (the registry uses `or_else` when applying classification).

use async_trait::async_trait;
use glpi_core::error::Result;
use std::sync::Arc;

use crate::snmp::query::SnmpQuery;
use crate::snmp::sysobject::SysObjectIds;

pub mod device;
pub mod if_mib;
pub mod system_mib;

pub use device::{DeviceInfo, NetworkDevice, Port};
pub use if_mib::IfMib;
pub use system_mib::SystemMib;

/// One MIB-support module: reads a slice of a device into [`NetworkDevice`].
#[async_trait]
pub trait MibSupport: Send + Sync {
    /// A short, stable identifier for logging.
    fn name(&self) -> &'static str;

    /// Reads this module's data from `session` into `device`.
    ///
    /// Modules set only the fields they can determine and must not overwrite
    /// values another module already populated.
    ///
    /// # Errors
    ///
    /// Propagates transport/protocol failures from `session`.
    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()>;
}

/// An ordered set of MIB modules driving a NetInventory.
#[derive(Clone, Default)]
pub struct MibRegistry {
    modules: Vec<Arc<dyn MibSupport>>,
}

impl MibRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry with all standard MIB modules implemented so far.
    #[must_use]
    pub fn with_standard() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(SystemMib));
        registry.register(Arc::new(IfMib));
        registry
    }

    /// Appends a module to run.
    pub fn register(&mut self, module: Arc<dyn MibSupport>) {
        self.modules.push(module);
    }

    /// Number of registered modules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// `true` if no modules are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Runs every module against `session`, then classifies the device from
    /// `sysobjects` (filling only manufacturer/type/model fields a module did
    /// not already set).
    ///
    /// # Errors
    ///
    /// Propagates the first module failure.
    pub async fn inventory(
        &self,
        session: &mut dyn SnmpQuery,
        sysobjects: &SysObjectIds,
    ) -> Result<NetworkDevice> {
        let mut device = NetworkDevice::default();
        for module in &self.modules {
            module.run(session, &mut device).await?;
        }
        if let Some(oid) = device.info.sys_object_id.clone() {
            if let Some(entry) = sysobjects.lookup(&oid) {
                let info = &mut device.info;
                info.manufacturer = info
                    .manufacturer
                    .take()
                    .or_else(|| entry.manufacturer.clone());
                info.r#type = info.r#type.take().or_else(|| entry.r#type.clone());
                info.model = info.model.take().or_else(|| entry.model.clone());
            }
        }
        Ok(device)
    }
}

/// Reads `oid` as a non-empty UTF-8 string, shared by MIB modules.
pub(crate) async fn get_string(session: &mut dyn SnmpQuery, oid: &[u64]) -> Result<Option<String>> {
    Ok(session
        .get(oid)
        .await?
        .and_then(|value| value.as_str())
        .filter(|s| !s.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::MibRegistry;
    use crate::snmp::sysobject::SysObjectIds;
    use crate::snmp::walk::WalkSession;

    const CISCO_WALK: &str = r#".1.3.6.1.2.1.1.1.0 = STRING: "Cisco IOS"
.1.3.6.1.2.1.1.2.0 = OID: .1.3.6.1.4.1.9.1.3
.1.3.6.1.2.1.1.5.0 = STRING: "sw-1"
"#;

    #[tokio::test]
    async fn inventory_runs_standard_mibs_and_classifies() {
        let mut session = WalkSession::parse(CISCO_WALK).unwrap();
        let sysobjects = SysObjectIds::parse("9.1.3\tCisco\tNETWORKING\tCatalyst 2960\n");
        let registry = MibRegistry::with_standard();
        assert_eq!(registry.len(), 2);

        let device = registry.inventory(&mut session, &sysobjects).await.unwrap();
        assert_eq!(device.info.name.as_deref(), Some("sw-1"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Cisco"));
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.model.as_deref(), Some("Catalyst 2960"));
    }

    #[tokio::test]
    async fn inventory_without_classification_match_leaves_type_none() {
        let mut session = WalkSession::parse(CISCO_WALK).unwrap();
        let sysobjects = SysObjectIds::default();
        let device = MibRegistry::with_standard()
            .inventory(&mut session, &sysobjects)
            .await
            .unwrap();
        assert_eq!(device.info.manufacturer, None);
        assert_eq!(device.info.r#type, None);
    }
}
