// SPDX-License-Identifier: GPL-2.0-only

//! TP-Link vendor MIB support (networking).
//!
//! Applies to TP-Link devices (`1.3.6.1.4.1.11863`) and reads the
//! `TPLINK-SYSINFO-MIB` system-info group: firmware, serial and model (the
//! first token of the hardware version), recording the hardware version as a
//! firmware entry. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::TpLink`; the legacy `sysObjectID`-relative
//! fallback and the VLAN-port enrichment are not modelled.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// TP-Link enterprise OID.
const TPLINK: &str = "1.3.6.1.4.1.11863";
/// `tpSysInfoHwVersion` (`tplinkSysInfoMIBObjects.5.0`).
const TP_SYS_INFO_HW_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 11863, 6, 1, 1, 5];
/// `tpSysInfoSwVersion` (`tplinkSysInfoMIBObjects.6.0`).
const TP_SYS_INFO_SW_VERSION: [u64; 11] = [1, 3, 6, 1, 4, 1, 11863, 6, 1, 1, 6];
/// `tpSysInfoSerialNum` (`tplinkSysInfoMIBObjects.8.0`).
const TP_SYS_INFO_SERIAL_NUM: [u64; 11] = [1, 3, 6, 1, 4, 1, 11863, 6, 1, 1, 8];

/// Vendor MIB module for TP-Link devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct TpLinkMib;

#[async_trait]
impl MibSupport for TpLinkMib {
    fn name(&self) -> &'static str {
        "tplink"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), TPLINK)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.firmware.is_none() {
            device.info.firmware = scalar(session, &TP_SYS_INFO_SW_VERSION).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = scalar(session, &TP_SYS_INFO_SERIAL_NUM).await?;
        }

        let hw_version = scalar(session, &TP_SYS_INFO_HW_VERSION).await?;
        if device.info.model.is_none() {
            device.info.model = hw_version
                .as_deref()
                .and_then(|hw| hw.split_whitespace().next())
                .map(str::to_owned);
        }
        if let Some(version) = hw_version {
            device.add_firmware(Firmware {
                name: device.info.model.clone(),
                description: Some("TP-Link Hardware version".to_owned()),
                r#type: Some("hardware".to_owned()),
                version: Some(version),
                manufacturer: Some("TP-Link".to_owned()),
            });
        }
        Ok(())
    }
}

/// Reads the `.0` instance of `oid` as a non-empty string.
async fn scalar(session: &mut dyn SnmpQuery, oid: &[u64]) -> Result<Option<String>> {
    let full: Vec<u64> = oid.iter().copied().chain(std::iter::once(0)).collect();
    get_string(session, &full).await
}

#[cfg(test)]
mod tests {
    use super::TpLinkMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_tplink() {
        assert!(TpLinkMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.11863.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!TpLinkMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.11864".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_hardware_firmware() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.11863.6.1.1.5.0 = STRING: \"T1600G-28TS 2.0\"\n\
             .1.3.6.1.4.1.11863.6.1.1.6.0 = STRING: \"2.0.5 Build 20180929\"\n\
             .1.3.6.1.4.1.11863.6.1.1.8.0 = STRING: \"2160230001\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        TpLinkMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(
            device.info.firmware.as_deref(),
            Some("2.0.5 Build 20180929")
        );
        assert_eq!(device.info.serial.as_deref(), Some("2160230001"));
        assert_eq!(device.info.model.as_deref(), Some("T1600G-28TS"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(device.firmwares[0].name.as_deref(), Some("T1600G-28TS"));
        assert_eq!(
            device.firmwares[0].version.as_deref(),
            Some("T1600G-28TS 2.0")
        );
    }
}
