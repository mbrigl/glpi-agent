// SPDX-License-Identifier: GPL-2.0-only

//! D-Link vendor MIB support (networking switches).
//!
//! Applies to D-Link products (`1.3.6.1.4.1.171.10`). D-Link exposes its
//! private scalars relative to the device's own `sysObjectID`, so this module
//! reads firmware, serial, hostname and hardware revision by appending the
//! documented sub-OIDs to the reported `sysObjectID`. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Dlink`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `dlink_products` (`d_link.10`) — the D-Link product OID subtree.
const DLINK_PRODUCTS: &str = "1.3.6.1.4.1.171.10";
/// `sysHostname` sub-OID (relative to the device `sysObjectID`).
const SUB_HOSTNAME: [u64; 3] = [1, 1, 0];
/// `sysHardwareVersion` sub-OID.
const SUB_HARDWARE_VERSION: [u64; 3] = [1, 2, 0];
/// `sysFirmwareVersion` sub-OID.
const SUB_FIRMWARE_VERSION: [u64; 3] = [1, 3, 0];
/// `sysSerialNumber` sub-OID.
const SUB_SERIAL_NUMBER: [u64; 3] = [1, 18, 0];

/// Vendor MIB module for D-Link switches.
#[derive(Debug, Default, Clone, Copy)]
pub struct DlinkMib;

#[async_trait]
impl MibSupport for DlinkMib {
    fn name(&self) -> &'static str {
        "d-link"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), DLINK_PRODUCTS)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let Some(base) = device.info.sys_object_id.as_deref().and_then(parse_oid) else {
            return Ok(());
        };

        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("D-Link".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = private(session, &base, &SUB_FIRMWARE_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = private(session, &base, &SUB_SERIAL_NUMBER).await?;
        }
        if device.info.name.is_none() {
            device.info.name = private(session, &base, &SUB_HOSTNAME).await?;
        }

        if let Some(version) = private(session, &base, &SUB_HARDWARE_VERSION).await? {
            let prefix = device
                .info
                .model
                .as_deref()
                .map_or(String::new(), |m| format!("{m} "));
            device.add_firmware(Firmware {
                name: Some(format!("{prefix}hardware")),
                description: Some("hardware revision".to_owned()),
                r#type: Some("device".to_owned()),
                version: Some(version),
                manufacturer: Some("D-Link".to_owned()),
            });
        }
        Ok(())
    }
}

/// Reads a D-Link private scalar (`sysObjectID` base + `suboid`) as a string.
async fn private(
    session: &mut dyn SnmpQuery,
    base: &[u64],
    suboid: &[u64],
) -> Result<Option<String>> {
    let oid: Vec<u64> = base.iter().chain(suboid).copied().collect();
    get_string(session, &oid).await
}

/// Parses a dotted `sysObjectID` (optionally leading-dot) into its arcs.
fn parse_oid(oid: &str) -> Option<Vec<u64>> {
    let arcs: Vec<u64> = oid
        .trim_start_matches('.')
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .ok()?;
    (!arcs.is_empty()).then_some(arcs)
}

#[cfg(test)]
mod tests {
    use super::DlinkMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    fn dlink_device(sys_object_id: &str) -> NetworkDevice {
        NetworkDevice {
            info: DeviceInfo {
                sys_object_id: Some(sys_object_id.to_owned()),
                model: Some("DGS-3120".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        }
    }

    #[test]
    fn applies_only_to_dlink_products() {
        assert!(DlinkMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.171.10.76.28".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!DlinkMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.171.11".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_private_scalars_relative_to_sysobjectid() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.171.10.76.28.1.1.0 = STRING: \"switch-core-1\"\n\
             .1.3.6.1.4.1.171.10.76.28.1.2.0 = STRING: \"B1\"\n\
             .1.3.6.1.4.1.171.10.76.28.1.3.0 = STRING: \"4.50.B021\"\n\
             .1.3.6.1.4.1.171.10.76.28.1.18.0 = STRING: \"R3173B5000123\"\n",
        )
        .unwrap();
        let mut device = dlink_device("1.3.6.1.4.1.171.10.76.28");
        DlinkMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("D-Link"));
        assert_eq!(device.info.firmware.as_deref(), Some("4.50.B021"));
        assert_eq!(device.info.serial.as_deref(), Some("R3173B5000123"));
        assert_eq!(device.info.name.as_deref(), Some("switch-core-1"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("DGS-3120 hardware")
        );
        assert_eq!(device.firmwares[0].version.as_deref(), Some("B1"));
    }
}
