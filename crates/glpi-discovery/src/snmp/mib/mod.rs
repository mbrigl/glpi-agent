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
use glpi_core::types::network::MacAddress;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::snmp::query::SnmpQuery;
use crate::snmp::sysobject::SysObjectIds;
use crate::snmp::value::SnmpValue;

pub mod bridge_mib;
pub mod cdp_mib;
pub mod device;
pub mod entity_mib;
pub mod if_mib;
pub mod ip_mib;
pub mod lldp_mib;
pub mod printer_mib;
pub mod system_mib;
pub mod vendor;

pub use bridge_mib::BridgeMib;
pub use cdp_mib::CdpMib;
pub use device::{
    Component, DeviceInfo, Neighbor, NeighborProtocol, NetworkDevice, Port, Printer, Supply,
};
pub use entity_mib::EntityMib;
pub use if_mib::IfMib;
pub use ip_mib::IpMib;
pub use lldp_mib::LldpMib;
pub use printer_mib::PrinterMib;
pub use system_mib::SystemMib;
pub use vendor::CiscoMib;

/// One MIB-support module: reads a slice of a device into [`NetworkDevice`].
#[async_trait]
pub trait MibSupport: Send + Sync {
    /// A short, stable identifier for logging.
    fn name(&self) -> &'static str;

    /// Whether this module applies to the device.
    ///
    /// Standard MIBs use the default (`true`, always run). Vendor MIBs override
    /// this to match on `sysObjectID` (see [`sysobjectid_matches`]). It is
    /// evaluated after the system group has been read, so `info.sys_object_id`
    /// is already populated for modules registered after [`SystemMib`].
    fn applies_to(&self, info: &DeviceInfo) -> bool {
        let _ = info;
        true
    }

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
        registry.register(Arc::new(EntityMib));
        registry.register(Arc::new(PrinterMib));
        registry.register(Arc::new(BridgeMib));
        registry.register(Arc::new(LldpMib));
        registry.register(Arc::new(CdpMib));
        registry.register(Arc::new(IpMib));
        registry
    }

    /// Creates a registry with the standard MIBs plus all vendor MIBs. Vendor
    /// modules are gated by `sysObjectID`, so this is the right default for a
    /// real inventory.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::with_standard();
        vendor::register_all(&mut registry);
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
            if module.applies_to(&device.info) {
                module.run(session, &mut device).await?;
            }
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

/// Walks a single-index table column and applies `set` to each row, creating
/// rows on demand via `new_row` keyed by the trailing index arc. Shared by the
/// table-oriented MIB modules (`if`, `entity`, …).
pub(crate) async fn apply_column<T, F>(
    session: &mut dyn SnmpQuery,
    base: &[u64],
    rows: &mut BTreeMap<u64, T>,
    new_row: fn(u64) -> T,
    set: F,
) -> Result<()>
where
    F: Fn(&mut T, SnmpValue),
{
    for (oid, value) in session.walk(base).await? {
        if let Some(index) = table_index(&oid, base) {
            set(rows.entry(index).or_insert_with(|| new_row(index)), value);
        }
    }
    Ok(())
}

/// Like [`apply_column`] but keys rows by the full instance *suffix* (the arcs
/// beyond `base`), for tables whose index spans more than one arc (e.g.
/// `prtMarkerSuppliesTable`, indexed by `hrDeviceIndex.suppliesIndex`).
pub(crate) async fn apply_suffix_column<T, N, F>(
    session: &mut dyn SnmpQuery,
    base: &[u64],
    rows: &mut BTreeMap<Vec<u64>, T>,
    new_row: N,
    set: F,
) -> Result<()>
where
    N: Fn(&[u64]) -> T,
    F: Fn(&mut T, SnmpValue),
{
    for (oid, value) in session.walk(base).await? {
        if let Some(suffix) = instance_suffix(&oid, base) {
            set(
                rows.entry(suffix.clone())
                    .or_insert_with(|| new_row(&suffix)),
                value,
            );
        }
    }
    Ok(())
}

/// Returns `true` if a device's `sys_object_id` falls under `enterprise_prefix`
/// (an exact match or a dotted-arc-boundary prefix), used by vendor MIBs to
/// declare applicability. `1.3.6.1.4.1.9` matches `…9` and `…9.1.3`, not `…99`.
#[must_use]
pub fn sysobjectid_matches(sys_object_id: Option<&str>, enterprise_prefix: &str) -> bool {
    match sys_object_id {
        Some(oid) => oid == enterprise_prefix || oid.starts_with(&format!("{enterprise_prefix}.")),
        None => false,
    }
}

/// Returns the instance suffix of `oid` under `base` (the arcs beyond `base`),
/// or `None` if `oid` is not a strict descendant.
pub(crate) fn instance_suffix(oid: &[u64], base: &[u64]) -> Option<Vec<u64>> {
    (oid.len() > base.len() && oid.starts_with(base)).then(|| oid[base.len()..].to_vec())
}

/// Finds the port with `index`, creating and appending one if absent. The
/// returned reference is to the (possibly new) port; callers that add ports
/// should re-sort `device.ports` afterwards.
pub(crate) fn port_mut(device: &mut NetworkDevice, index: u64) -> &mut Port {
    if let Some(pos) = device.ports.iter().position(|p| p.index == index) {
        &mut device.ports[pos]
    } else {
        device.ports.push(Port::new(index));
        device.ports.last_mut().expect("just pushed")
    }
}

/// Returns the single-arc table index of `oid` under `base` (one arc beyond it).
pub(crate) fn table_index(oid: &[u64], base: &[u64]) -> Option<u64> {
    if oid.len() == base.len() + 1 && oid.starts_with(base) {
        Some(oid[base.len()])
    } else {
        None
    }
}

/// Extracts a signed integer value.
pub(crate) fn as_i64(value: &SnmpValue) -> Option<i64> {
    match value {
        SnmpValue::Integer(n) => Some(*n),
        _ => None,
    }
}

/// Extracts a signed number from an integer or any counter/gauge type.
pub(crate) fn as_number(value: &SnmpValue) -> Option<i64> {
    as_i64(value).or_else(|| as_u64(value).and_then(|n| i64::try_from(n).ok()))
}

/// Extracts an unsigned value from any of the counter/gauge/integer types.
pub(crate) fn as_u64(value: &SnmpValue) -> Option<u64> {
    match value {
        SnmpValue::Unsigned32(n) | SnmpValue::Counter32(n) | SnmpValue::Timeticks(n) => {
            Some(u64::from(*n))
        }
        SnmpValue::Counter64(n) => Some(*n),
        SnmpValue::Integer(n) if *n >= 0 => Some(*n as u64),
        _ => None,
    }
}

/// Extracts a six-octet MAC from an `OCTET STRING`, rejecting the all-zero one.
pub(crate) fn as_mac(value: &SnmpValue) -> Option<MacAddress> {
    let SnmpValue::OctetString(bytes) = value else {
        return None;
    };
    let octets: [u8; 6] = bytes.as_slice().try_into().ok()?;
    (octets != [0u8; 6]).then(|| MacAddress::new(octets))
}

#[cfg(test)]
mod tests {
    use super::{
        sysobjectid_matches, DeviceInfo, MibRegistry, MibSupport, NetworkDevice, SystemMib,
    };
    use crate::snmp::query::SnmpQuery;
    use crate::snmp::sysobject::SysObjectIds;
    use crate::snmp::walk::WalkSession;
    use async_trait::async_trait;
    use glpi_core::error::Result;
    use std::sync::Arc;

    const CISCO_WALK: &str = r#".1.3.6.1.2.1.1.1.0 = STRING: "Cisco IOS"
.1.3.6.1.2.1.1.2.0 = OID: .1.3.6.1.4.1.9.1.3
.1.3.6.1.2.1.1.5.0 = STRING: "sw-1"
"#;

    /// A vendor MIB that applies only to Cisco and tags the model, used to
    /// exercise the sysObjectID gating.
    struct CiscoTag;

    #[async_trait]
    impl MibSupport for CiscoTag {
        fn name(&self) -> &'static str {
            "cisco-tag"
        }
        fn applies_to(&self, info: &DeviceInfo) -> bool {
            sysobjectid_matches(info.sys_object_id.as_deref(), "1.3.6.1.4.1.9")
        }
        async fn run(
            &self,
            _session: &mut dyn SnmpQuery,
            device: &mut NetworkDevice,
        ) -> Result<()> {
            device.info.model = Some("tagged-by-vendor-mib".to_owned());
            Ok(())
        }
    }

    fn registry_with_cisco_tag() -> MibRegistry {
        let mut registry = MibRegistry::new();
        registry.register(Arc::new(SystemMib)); // must run first to set sys_object_id
        registry.register(Arc::new(CiscoTag));
        registry
    }

    #[test]
    fn sysobjectid_matching_is_arc_boundary_aware() {
        assert!(sysobjectid_matches(Some("1.3.6.1.4.1.9"), "1.3.6.1.4.1.9"));
        assert!(sysobjectid_matches(
            Some("1.3.6.1.4.1.9.1.3"),
            "1.3.6.1.4.1.9"
        ));
        // 99 must not match the prefix 9.
        assert!(!sysobjectid_matches(
            Some("1.3.6.1.4.1.99"),
            "1.3.6.1.4.1.9"
        ));
        assert!(!sysobjectid_matches(None, "1.3.6.1.4.1.9"));
    }

    #[tokio::test]
    async fn vendor_mib_runs_only_for_matching_sysobjectid() {
        let registry = registry_with_cisco_tag();
        let sysobjects = SysObjectIds::default();

        // Cisco device: the vendor MIB applies and tags the model.
        let mut cisco = WalkSession::parse(CISCO_WALK).unwrap();
        let device = registry.inventory(&mut cisco, &sysobjects).await.unwrap();
        assert_eq!(device.info.model.as_deref(), Some("tagged-by-vendor-mib"));

        // Juniper device: the Cisco MIB is skipped.
        let mut juniper = WalkSession::parse(
            ".1.3.6.1.2.1.1.1.0 = STRING: \"Juniper\"\n\
             .1.3.6.1.2.1.1.2.0 = OID: .1.3.6.1.4.1.2636.1.1\n",
        )
        .unwrap();
        let device = registry.inventory(&mut juniper, &sysobjects).await.unwrap();
        assert_eq!(device.info.model, None);
    }

    #[tokio::test]
    async fn inventory_runs_standard_mibs_and_classifies() {
        let mut session = WalkSession::parse(CISCO_WALK).unwrap();
        let sysobjects = SysObjectIds::parse("9.1.3\tCisco\tNETWORKING\tCatalyst 2960\n");
        let registry = MibRegistry::with_standard();
        assert_eq!(registry.len(), 8);

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
