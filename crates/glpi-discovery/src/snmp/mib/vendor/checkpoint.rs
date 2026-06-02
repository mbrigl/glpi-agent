// SPDX-License-Identifier: GPL-2.0-only

//! Check Point appliance vendor MIB support.
//!
//! Applies to Check Point devices (`1.3.6.1.4.1.2620`) and reads the SVN
//! appliance group: firmware (`<version> (build <build>)`), serial,
//! manufacturer and model, plus the product SVN and OS versions as firmware
//! entries. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::CheckPoint`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// Check Point enterprise OID.
const CHECKPOINT: &str = "1.3.6.1.4.1.2620";
/// `svnProdName` (`svn.1.0`).
const SVN_PROD_NAME: [u64; 11] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 1, 0];
/// `svnProdVerMajor` (`svn.2.0`).
const SVN_PROD_VER_MAJOR: [u64; 11] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 2, 0];
/// `svnProdVerMinor` (`svn.3.0`).
const SVN_PROD_VER_MINOR: [u64; 11] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 3, 0];
/// `svnVersion` (`svnInfo.1.0`).
const SVN_VERSION: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 4, 1, 0];
/// `svnBuild` (`svnInfo.2.0`).
const SVN_BUILD: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 4, 2, 0];
/// `osName` (`svnOSInfo.1.0`).
const OS_NAME: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 5, 1, 0];
/// `osMajorVer` (`svnOSInfo.2.0`).
const OS_MAJOR_VER: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 5, 2, 0];
/// `osMinorVer` (`svnOSInfo.3.0`).
const OS_MINOR_VER: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 5, 3, 0];
/// `svnApplianceSerialNumber` (`svnApplianceInfo.3.0`).
const SVN_APPLIANCE_SERIAL: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 16, 3, 0];
/// `svnApplianceModel` (`svnApplianceInfo.7.0`).
const SVN_APPLIANCE_MODEL: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 16, 7, 0];
/// `svnApplianceManufacturer` (`svnApplianceInfo.9.0`).
const SVN_APPLIANCE_MANUFACTURER: [u64; 12] = [1, 3, 6, 1, 4, 1, 2620, 1, 6, 16, 9, 0];

/// Vendor MIB module for Check Point appliances.
#[derive(Debug, Default, Clone, Copy)]
pub struct CheckPointMib;

#[async_trait]
impl MibSupport for CheckPointMib {
    fn name(&self) -> &'static str {
        "CheckPoint"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), CHECKPOINT)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        let manufacturer = get_string(session, &SVN_APPLIANCE_MANUFACTURER).await?;
        if device.info.manufacturer.is_none() {
            device.info.manufacturer.clone_from(&manufacturer);
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &SVN_APPLIANCE_SERIAL).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &SVN_APPLIANCE_MODEL).await?;
        }
        if device.info.firmware.is_none() {
            if let Some(version) = get_string(session, &SVN_VERSION).await? {
                let build = get_string(session, &SVN_BUILD).await?.unwrap_or_default();
                device.info.firmware = Some(format!("{version} (build {build})"));
            }
        }

        let manufacturer = manufacturer.unwrap_or_else(|| "Check Point".to_owned());
        if let Some(version) =
            version_pair(session, &SVN_PROD_VER_MAJOR, &SVN_PROD_VER_MINOR).await?
        {
            device.add_firmware(firmware(
                get_string(session, &SVN_PROD_NAME).await?,
                &format!("{manufacturer} SVN version"),
                version,
                &manufacturer,
            ));
        }
        if let Some(version) = version_pair(session, &OS_MAJOR_VER, &OS_MINOR_VER).await? {
            device.add_firmware(firmware(
                get_string(session, &OS_NAME).await?,
                &format!("{manufacturer} OS version"),
                version,
                &manufacturer,
            ));
        }
        Ok(())
    }
}

/// Joins a `<major>.<minor>` version when the major part is present.
async fn version_pair(
    session: &mut dyn SnmpQuery,
    major: &[u64],
    minor: &[u64],
) -> Result<Option<String>> {
    let Some(major) = get_string(session, major).await? else {
        return Ok(None);
    };
    let minor = get_string(session, minor).await?.unwrap_or_default();
    Ok(Some(format!("{major}.{minor}")))
}

/// Builds a Check Point system firmware entry.
fn firmware(
    name: Option<String>,
    description: &str,
    version: String,
    manufacturer: &str,
) -> Firmware {
    Firmware {
        name,
        description: Some(description.to_owned()),
        r#type: Some("system".to_owned()),
        version: Some(version),
        manufacturer: Some(manufacturer.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::CheckPointMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_checkpoint() {
        assert!(CheckPointMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2620.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!CheckPointMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.2621".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_version_firmwares() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.2620.1.6.1.0 = STRING: \"SVN Foundation\"\n\
             .1.3.6.1.4.1.2620.1.6.2.0 = STRING: \"R81\"\n\
             .1.3.6.1.4.1.2620.1.6.3.0 = STRING: \"10\"\n\
             .1.3.6.1.4.1.2620.1.6.4.1.0 = STRING: \"R81\"\n\
             .1.3.6.1.4.1.2620.1.6.4.2.0 = STRING: \"392\"\n\
             .1.3.6.1.4.1.2620.1.6.5.1.0 = STRING: \"Gaia\"\n\
             .1.3.6.1.4.1.2620.1.6.5.2.0 = STRING: \"3\"\n\
             .1.3.6.1.4.1.2620.1.6.5.3.0 = STRING: \"10\"\n\
             .1.3.6.1.4.1.2620.1.6.16.3.0 = STRING: \"1234CK5678\"\n\
             .1.3.6.1.4.1.2620.1.6.16.7.0 = STRING: \"Check Point 6200\"\n\
             .1.3.6.1.4.1.2620.1.6.16.9.0 = STRING: \"Check Point\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        CheckPointMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.manufacturer.as_deref(), Some("Check Point"));
        assert_eq!(device.info.serial.as_deref(), Some("1234CK5678"));
        assert_eq!(device.info.model.as_deref(), Some("Check Point 6200"));
        assert_eq!(device.info.firmware.as_deref(), Some("R81 (build 392)"));
        assert_eq!(device.firmwares.len(), 2);
        assert_eq!(device.firmwares[0].version.as_deref(), Some("R81.10"));
        assert_eq!(device.firmwares[1].name.as_deref(), Some("Gaia"));
        assert_eq!(device.firmwares[1].version.as_deref(), Some("3.10"));
    }
}
