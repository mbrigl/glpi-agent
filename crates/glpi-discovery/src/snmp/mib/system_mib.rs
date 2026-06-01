// SPDX-License-Identifier: GPL-2.0-only

//! Standard `SNMPv2-MIB` system group support.
//!
//! Reads the always-present system group (`sysDescr`, `sysObjectID`,
//! `sysUpTime`, `sysContact`, `sysName`, `sysLocation`, RFC 1213 / RFC 3418)
//! into [`DeviceInfo`]. This is the first MIB run for every device and the
//! source of the `sysObjectID` used to classify it and to select vendor MIBs.

use async_trait::async_trait;
use glpi_core::error::Result;

use super::{get_string, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;
use crate::snmp::value::SnmpValue;

const SYS_DESCR: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 1, 0];
const SYS_OBJECT_ID: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 2, 0];
const SYS_UPTIME: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 3, 0];
const SYS_CONTACT: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 4, 0];
const SYS_NAME: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 5, 0];
const SYS_LOCATION: [u64; 9] = [1, 3, 6, 1, 2, 1, 1, 6, 0];

/// MIB module for the standard system group.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMib;

#[async_trait]
impl MibSupport for SystemMib {
    fn name(&self) -> &'static str {
        "system"
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let info = &mut device.info;
        info.description = get_string(session, &SYS_DESCR).await?;
        info.name = get_string(session, &SYS_NAME).await?;
        info.contact = get_string(session, &SYS_CONTACT).await?;
        info.location = get_string(session, &SYS_LOCATION).await?;

        info.sys_object_id = session
            .get(&SYS_OBJECT_ID)
            .await?
            .and_then(|value| match value {
                SnmpValue::Oid(oid) => Some(oid),
                _ => None,
            });
        info.uptime = session
            .get(&SYS_UPTIME)
            .await?
            .and_then(|value| match value {
                SnmpValue::Timeticks(ticks) => Some(ticks),
                _ => None,
            });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SystemMib;
    use crate::snmp::mib::{MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    const WALK: &str = r#".1.3.6.1.2.1.1.1.0 = STRING: "Juniper EX2200"
.1.3.6.1.2.1.1.2.0 = OID: .1.3.6.1.4.1.2636.1.1.1.2.30
.1.3.6.1.2.1.1.3.0 = Timeticks: (98765) 0:16:27.65
.1.3.6.1.2.1.1.4.0 = STRING: "noc@example.net"
.1.3.6.1.2.1.1.5.0 = STRING: "edge-1"
.1.3.6.1.2.1.1.6.0 = STRING: "DC2 row B"
"#;

    #[tokio::test]
    async fn populates_device_info_from_system_group() {
        let mut session = WalkSession::parse(WALK).unwrap();
        let mut device = NetworkDevice::default();
        SystemMib.run(&mut session, &mut device).await.unwrap();

        let info = device.info;
        assert_eq!(info.description.as_deref(), Some("Juniper EX2200"));
        assert_eq!(info.name.as_deref(), Some("edge-1"));
        assert_eq!(info.contact.as_deref(), Some("noc@example.net"));
        assert_eq!(info.location.as_deref(), Some("DC2 row B"));
        assert_eq!(info.uptime, Some(98765));
        assert_eq!(
            info.sys_object_id.as_deref(),
            Some("1.3.6.1.4.1.2636.1.1.1.2.30")
        );
    }

    #[tokio::test]
    async fn leaves_fields_none_for_a_bare_device() {
        let mut session = WalkSession::parse("").unwrap();
        let mut device = NetworkDevice::default();
        SystemMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info, crate::snmp::mib::DeviceInfo::default());
    }
}
