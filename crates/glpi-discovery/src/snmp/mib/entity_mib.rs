// SPDX-License-Identifier: GPL-2.0-only

//! Standard `ENTITY-MIB` physical-component support.
//!
//! Walks `entPhysicalTable` (RFC 4133) to build the device's [`Component`]
//! list — chassis, modules, power supplies, fans, CPUs — and promotes the
//! chassis entry's serial / model / manufacturer / firmware to the device-level
//! [`DeviceInfo`] (without overwriting values another module already set).

use std::collections::BTreeMap;

use async_trait::async_trait;
use glpi_core::error::Result;

use super::{apply_column, as_i64, Component, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

// entPhysicalTable columns (1.3.6.1.2.1.47.1.1.1.1.N).
const ENT_DESCR: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 2];
const ENT_CLASS: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 5];
const ENT_NAME: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 7];
const ENT_HARDWARE_REV: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 8];
const ENT_FIRMWARE_REV: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 9];
const ENT_SOFTWARE_REV: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 10];
const ENT_SERIAL_NUM: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 11];
const ENT_MFG_NAME: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 12];
const ENT_MODEL_NAME: [u64; 12] = [1, 3, 6, 1, 2, 1, 47, 1, 1, 1, 1, 13];

/// MIB module for the standard physical-entity table.
#[derive(Debug, Default, Clone, Copy)]
pub struct EntityMib;

#[async_trait]
impl MibSupport for EntityMib {
    fn name(&self) -> &'static str {
        "entity"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let mut components: BTreeMap<u64, Component> = BTreeMap::new();

        apply_column(
            session,
            &ENT_DESCR,
            &mut components,
            Component::new,
            |c, v| {
                c.description = v.as_str();
            },
        )
        .await?;
        apply_column(
            session,
            &ENT_CLASS,
            &mut components,
            Component::new,
            |c, v| {
                c.class = as_i64(&v);
            },
        )
        .await?;
        apply_column(
            session,
            &ENT_NAME,
            &mut components,
            Component::new,
            |c, v| {
                c.name = v.as_str();
            },
        )
        .await?;
        apply_column(
            session,
            &ENT_HARDWARE_REV,
            &mut components,
            Component::new,
            |c, v| c.hardware_rev = v.as_str(),
        )
        .await?;
        apply_column(
            session,
            &ENT_FIRMWARE_REV,
            &mut components,
            Component::new,
            |c, v| c.firmware = v.as_str(),
        )
        .await?;
        apply_column(
            session,
            &ENT_SOFTWARE_REV,
            &mut components,
            Component::new,
            |c, v| c.software_rev = v.as_str(),
        )
        .await?;
        apply_column(
            session,
            &ENT_SERIAL_NUM,
            &mut components,
            Component::new,
            |c, v| c.serial = v.as_str(),
        )
        .await?;
        apply_column(
            session,
            &ENT_MFG_NAME,
            &mut components,
            Component::new,
            |c, v| c.manufacturer = v.as_str(),
        )
        .await?;
        apply_column(
            session,
            &ENT_MODEL_NAME,
            &mut components,
            Component::new,
            |c, v| c.model = v.as_str(),
        )
        .await?;

        device.components = components.into_values().collect();
        promote_chassis(device);
        Ok(())
    }
}

/// Copies the chassis component's identity to the device level, without
/// overwriting fields a prior module already set.
fn promote_chassis(device: &mut NetworkDevice) {
    let Some(chassis) = device
        .components
        .iter()
        .find(|c| c.class == Some(Component::CLASS_CHASSIS))
    else {
        return;
    };
    let info = &mut device.info;
    if info.serial.is_none() {
        info.serial = chassis.serial.clone();
    }
    if info.model.is_none() {
        info.model = chassis.model.clone();
    }
    if info.manufacturer.is_none() {
        info.manufacturer = chassis.manufacturer.clone();
    }
    if info.firmware.is_none() {
        info.firmware = chassis
            .firmware
            .clone()
            .or_else(|| chassis.software_rev.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::EntityMib;
    use crate::snmp::mib::{Component, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    const ENTITY_WALK: &str = r#".1.3.6.1.2.1.47.1.1.1.1.2.1 = STRING: "Catalyst 2960 chassis"
.1.3.6.1.2.1.47.1.1.1.1.2.2 = STRING: "Power Supply 1"
.1.3.6.1.2.1.47.1.1.1.1.5.1 = INTEGER: 3
.1.3.6.1.2.1.47.1.1.1.1.5.2 = INTEGER: 6
.1.3.6.1.2.1.47.1.1.1.1.9.1 = STRING: "12.2(25)"
.1.3.6.1.2.1.47.1.1.1.1.11.1 = STRING: "FOC1234X5YZ"
.1.3.6.1.2.1.47.1.1.1.1.12.1 = STRING: "Cisco"
.1.3.6.1.2.1.47.1.1.1.1.13.1 = STRING: "WS-C2960-24TT-L"
"#;

    async fn run() -> NetworkDevice {
        let mut session = WalkSession::parse(ENTITY_WALK).unwrap();
        let mut device = NetworkDevice::default();
        EntityMib.run(&mut session, &mut device).await.unwrap();
        device
    }

    #[tokio::test]
    async fn builds_components_ordered_by_index() {
        let device = run().await;
        assert_eq!(device.components.len(), 2);
        assert_eq!(device.components[0].class, Some(Component::CLASS_CHASSIS));
        assert_eq!(device.components[0].serial.as_deref(), Some("FOC1234X5YZ"));
        assert_eq!(
            device.components[1].description.as_deref(),
            Some("Power Supply 1")
        );
    }

    #[tokio::test]
    async fn promotes_chassis_identity_to_device_info() {
        let info = run().await.info;
        assert_eq!(info.serial.as_deref(), Some("FOC1234X5YZ"));
        assert_eq!(info.model.as_deref(), Some("WS-C2960-24TT-L"));
        assert_eq!(info.manufacturer.as_deref(), Some("Cisco"));
        assert_eq!(info.firmware.as_deref(), Some("12.2(25)"));
    }

    #[tokio::test]
    async fn no_chassis_leaves_device_info_untouched() {
        let mut session = WalkSession::parse(".1.3.6.1.2.1.47.1.1.1.1.5.1 = INTEGER: 6\n").unwrap();
        let mut device = NetworkDevice::default();
        EntityMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.components.len(), 1);
        assert_eq!(device.info.serial, None);
        assert_eq!(device.info.model, None);
    }
}
