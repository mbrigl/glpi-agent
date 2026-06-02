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
use glpi_iec61850::{IedIdentity, IedProtocol};

use crate::snmp::client::{SnmpClient, SNMP_PORT};
use crate::snmp::mib::{Firmware, MibRegistry, NetworkDevice};
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
    /// Creates a task that tries `credentials` in order, running the standard
    /// MIBs plus the (sysObjectID-gated) vendor MIBs. Defaults: UDP 161,
    /// 1-second per-request timeout, no retries, empty `sysobject.ids`.
    #[must_use]
    pub fn new(credentials: Vec<SnmpCredentials>) -> Self {
        Self {
            credentials: Arc::from(credentials),
            registry: MibRegistry::with_defaults(),
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

    /// Sets the target GLPI version, enabling version-dependent output such as
    /// the `PDU` device type for power-distribution units.
    #[must_use]
    pub fn with_glpi_version(mut self, glpi_version: impl Into<String>) -> Self {
        self.registry = self.registry.with_glpi_version(glpi_version);
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

    /// Inventories `target` over SNMP and IEC 61850, merging both into one
    /// [`NetworkDevice`] (the plan's "merge IEC 61850 + SNMP in NetInventory").
    ///
    /// SNMP runs first; the IED nameplate read over `ied` then fills in any
    /// identity fields SNMP left empty and contributes the IED firmware /
    /// hardware entries. A device that answers only IEC 61850 (no SNMP) is still
    /// inventoried from its nameplate; if neither responds, `None` is returned.
    ///
    /// # Errors
    ///
    /// Propagates a fatal SNMP error, or a transport error from `ied`.
    pub async fn inventory_with_ied<P: IedProtocol + ?Sized>(
        &self,
        target: IpAddr,
        ied: &mut P,
    ) -> Result<Option<NetworkDevice>> {
        let snmp = self.inventory(target).await?;
        let identity = IedIdentity::scan(ied).await?;

        match (snmp, identity.is_empty()) {
            (Some(mut device), false) => {
                merge_ied_identity(&mut device, &identity);
                Ok(Some(device))
            }
            (Some(device), true) => Ok(Some(device)),
            (None, false) => {
                // IEC-61850-only device: build it from the nameplate alone.
                let mut device = NetworkDevice::default();
                merge_ied_identity(&mut device, &identity);
                Ok(Some(device))
            }
            (None, true) => Ok(None),
        }
    }
}

/// Merges an [`IedIdentity`] into a [`NetworkDevice`] in place.
///
/// SNMP values take precedence: each identity field fills its `INFO` slot only
/// when SNMP left it empty. The IED's firmware and hardware revisions are always
/// appended as `FIRMWARES` entries.
pub fn merge_ied_identity(device: &mut NetworkDevice, identity: &IedIdentity) {
    let info = &mut device.info;
    fill(&mut info.manufacturer, identity.manufacturer.as_ref());
    fill(&mut info.model, identity.model.as_ref());
    fill(&mut info.serial, identity.serial.as_ref());
    fill(&mut info.firmware, identity.firmware.as_ref());
    fill(&mut info.contact, identity.contact.as_ref());
    fill(&mut info.location, identity.location.as_ref());
    if info.name.is_none() {
        info.name = identity.cleaned_name();
    }
    if info.r#type.is_none() {
        info.r#type = Some("NETWORKING".to_owned());
    }

    for entry in identity.firmware_entries() {
        device.add_firmware(Firmware {
            name: Some(entry.name),
            description: Some(entry.description),
            r#type: Some(entry.r#type),
            version: entry.version,
            manufacturer: entry.manufacturer,
        });
    }
}

/// Fills `slot` from `value` only when `slot` is still empty (SNMP wins).
fn fill(slot: &mut Option<String>, value: Option<&String>) {
    if slot.is_none() {
        *slot = value.cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_ied_identity, NetInventoryTask};
    use crate::snmp::mib::{DeviceInfo, NetworkDevice};
    use glpi_iec61850::{IedIdentity, MockProtocol};
    use std::net::{IpAddr, Ipv4Addr};

    fn full_identity() -> IedIdentity {
        IedIdentity {
            ied_name: Some("IED1A_Allg".to_owned()),
            manufacturer: Some("SIEMENS".to_owned()),
            model: Some("7SJ8221".to_owned()),
            serial: Some("BF1234567".to_owned()),
            firmware: Some("V07.80".to_owned()),
            hardware: Some("EE".to_owned()),
            contact: Some("Substation A".to_owned()),
            location: Some("Bay 3".to_owned()),
        }
    }

    #[tokio::test]
    async fn no_credentials_yields_no_device_without_touching_the_network() {
        let task = NetInventoryTask::new(Vec::new());
        let target = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)); // TEST-NET-1, never contacted
        assert_eq!(task.inventory(target).await.unwrap(), None);
    }

    #[test]
    fn merge_fills_empty_fields_and_keeps_snmp_values() {
        // SNMP already learned a name and manufacturer; the IED must not clobber
        // them, but should supply the model/serial/firmware SNMP lacked.
        let mut device = NetworkDevice {
            info: DeviceInfo {
                name: Some("snmp-name".to_owned()),
                manufacturer: Some("Acme".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        merge_ied_identity(&mut device, &full_identity());

        assert_eq!(device.info.name.as_deref(), Some("snmp-name")); // SNMP wins
        assert_eq!(device.info.manufacturer.as_deref(), Some("Acme")); // SNMP wins
        assert_eq!(device.info.model.as_deref(), Some("7SJ8221")); // from IED
        assert_eq!(device.info.serial.as_deref(), Some("BF1234567"));
        assert_eq!(device.info.location.as_deref(), Some("Bay 3"));
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));

        // Two FIRMWARES entries (firmware + hardware), with the GLPI keys.
        assert_eq!(device.firmwares.len(), 2);
        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["firmwares"][0]["name"], "7SJ8221 firmware");
        assert_eq!(json["firmwares"][0]["version"], "V07.80");
        assert_eq!(json["firmwares"][1]["name"], "7SJ8221 hardware");
    }

    #[test]
    fn merge_into_empty_device_uses_cleaned_ied_name() {
        let mut device = NetworkDevice::default();
        merge_ied_identity(&mut device, &full_identity());
        // The `A_Allg` suffix is stripped.
        assert_eq!(device.info.name.as_deref(), Some("IED1"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("SIEMENS"));
    }

    #[tokio::test]
    async fn iec_only_device_is_built_without_snmp() {
        // No SNMP credentials -> SNMP yields nothing; the mock IED still does.
        let task = NetInventoryTask::new(Vec::new());
        let mut ied = MockProtocol::new()
            .with_logical_device("IED2", &["LPHD1"])
            .with_data_objects("IED2/LPHD1", &["PhyNam"])
            .with_value("IED2/LPHD1.PhyNam.vendor", "ABB")
            .with_value("IED2/LPHD1.PhyNam.model", "REL670")
            .with_value("IED2/LPHD1.PhyNam.swRev", "2.2");

        let target = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2)); // TEST-NET-1
        let device = task
            .inventory_with_ied(target, &mut ied)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(device.info.name.as_deref(), Some("IED2"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("ABB"));
        assert_eq!(device.info.model.as_deref(), Some("REL670"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("2.2"));
    }

    #[tokio::test]
    async fn no_snmp_and_empty_ied_yields_none() {
        let task = NetInventoryTask::new(Vec::new());
        let mut ied = MockProtocol::new();
        let target = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 3));
        assert!(task
            .inventory_with_ied(target, &mut ied)
            .await
            .unwrap()
            .is_none());
    }
}
