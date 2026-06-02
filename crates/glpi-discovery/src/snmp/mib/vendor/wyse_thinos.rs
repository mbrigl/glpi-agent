// SPDX-License-Identifier: GPL-2.0-only

//! Dell Wyse ThinOS thin-client vendor MIB support (networking).
//!
//! Applies to Wyse thin clients (`WYSE-MIB ThinClient`, `1.3.6.1.4.1.714.1.2`)
//! and sets the `NETWORKING` type, the Dell manufacturer, the serial and the
//! model (`Wyse <first description word>`), recording the ThinOS version (the
//! remainder of the description) as a system firmware entry. Ported from the
//! upstream `GLPI::Agent::SNMP::MibSupport::WyseThinOS`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `ThinClient` (`wyse.1.2`) — the matched sysObjectID prefix.
const THIN_CLIENT: &str = "1.3.6.1.4.1.714.1.2";
/// `SerialNumber` (`ThinClient.6.2.1.0`).
const SERIAL_NUMBER: [u64; 13] = [1, 3, 6, 1, 4, 1, 714, 1, 2, 6, 2, 1, 0];

/// Vendor MIB module for Dell Wyse ThinOS thin clients.
#[derive(Debug, Default, Clone, Copy)]
pub struct WyseThinOsMib;

#[async_trait]
impl MibSupport for WyseThinOsMib {
    fn name(&self) -> &'static str {
        "wyse-thinos"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), THIN_CLIENT)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.manufacturer.is_none() {
            device.info.manufacturer = Some("Dell".to_owned());
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SERIAL_NUMBER).await?;
        }

        let description = device.info.description.clone().unwrap_or_default();
        let mut tokens = description.splitn(2, char::is_whitespace);
        let first = tokens.next().filter(|s| !s.is_empty());
        let rest = tokens.next().filter(|s| !s.is_empty());
        if device.info.model.is_none() {
            if let Some(first) = first {
                device.info.model = Some(format!("Wyse {first}"));
            }
        }
        if let Some(version) = rest {
            device.add_firmware(Firmware {
                name: Some("ThinOS".to_owned()),
                description: Some("Dell Wyse ThinOS".to_owned()),
                r#type: Some("system".to_owned()),
                version: Some(version.to_owned()),
                manufacturer: Some("Dell".to_owned()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WyseThinOsMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_wyse_thinclient() {
        assert!(WyseThinOsMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.714.1.2.6".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!WyseThinOsMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.714.1.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_thinos_firmware() {
        let mut session =
            WalkSession::parse(".1.3.6.1.4.1.714.1.2.6.2.1.0 = STRING: \"GMABC123\"\n").unwrap();
        let mut device = NetworkDevice {
            info: DeviceInfo {
                description: Some("5070 ThinOS 9.1.3129".to_owned()),
                ..DeviceInfo::default()
            },
            ..NetworkDevice::default()
        };
        WyseThinOsMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.manufacturer.as_deref(), Some("Dell"));
        assert_eq!(device.info.serial.as_deref(), Some("GMABC123"));
        assert_eq!(device.info.model.as_deref(), Some("Wyse 5070"));
        assert_eq!(device.firmwares.len(), 1);
        assert_eq!(
            device.firmwares[0].version.as_deref(),
            Some("ThinOS 9.1.3129")
        );
    }
}
