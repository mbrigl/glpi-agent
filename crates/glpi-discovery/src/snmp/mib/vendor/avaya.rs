// SPDX-License-Identifier: GPL-2.0-only

//! Avaya J100 IP phone vendor MIB support.
//!
//! Applies to Avaya 96x1 SIP endpoints (`1.3.6.1.4.1.6889.1.69.6`) and reads the
//! endpoint identity group. Sets the `NETWORKING` type, firmware, model and
//! serial, and records the DSP, hardware, OpenSSL and OpenSSH versions as
//! firmware entries. Ported from the upstream
//! `GLPI::Agent::SNMP::MibSupport::Avaya`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{
    get_string, sysobjectid_matches, DeviceInfo, Firmware, MibSupport, NetworkDevice,
};
use crate::snmp::query::SnmpQuery;

/// `avaya96x1SIPEndpoints` — the sysObjectID prefix of the supported phones.
const AVAYA_96X1_SIP: &str = "1.3.6.1.4.1.6889.1.69.6";
/// `endptAPPINUSE` (`endptID.4.0`) — the active firmware.
const ENDPT_APP_IN_USE: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 4, 0];
/// `endptDSPVERSION` (`endptID.27.0`).
const ENDPT_DSP_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 27, 0];
/// `endptMODEL` (`endptID.52.0`).
const ENDPT_MODEL: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 52, 0];
/// `endptPHONESN` (`endptID.57.0`).
const ENDPT_PHONE_SN: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 57, 0];
/// `endptHWVER` (`endptID.139.0`).
const ENDPT_HW_VER: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 139, 0];
/// `endptOpenSSLVersion` (`endptID.168.0`).
const ENDPT_OPENSSL_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 168, 0];
/// `endptOpenSSHVersion` (`endptID.169.0`).
const ENDPT_OPENSSH_VERSION: [u64; 13] = [1, 3, 6, 1, 4, 1, 6889, 2, 69, 6, 1, 169, 0];

/// Vendor MIB module for Avaya J100 IP phones.
#[derive(Debug, Default, Clone, Copy)]
pub struct AvayaMib;

#[async_trait]
impl MibSupport for AvayaMib {
    fn name(&self) -> &'static str {
        "avaya-j100-ipphone"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), AVAYA_96X1_SIP)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        if device.info.r#type.is_none() {
            device.info.r#type = Some("NETWORKING".to_owned());
        }
        if device.info.firmware.is_none() {
            device.info.firmware = get_string(session, &ENDPT_APP_IN_USE).await?;
        }
        if device.info.model.is_none() {
            device.info.model = get_string(session, &ENDPT_MODEL).await?;
        }
        if device.info.serial.is_none() {
            device.info.serial = get_string(session, &ENDPT_PHONE_SN).await?;
        }

        let model = device.info.model.clone().unwrap_or_default();
        for (oid, suffix, description, kind) in [
            (
                &ENDPT_DSP_VERSION,
                "DSP firmware",
                "DSP firmware version",
                "dsp",
            ),
            (&ENDPT_HW_VER, "Hardware", "Hardware version", "hardware"),
            (
                &ENDPT_OPENSSL_VERSION,
                "OpenSSL",
                "OpenSSL version",
                "software",
            ),
            (
                &ENDPT_OPENSSH_VERSION,
                "OpenSSH",
                "OpenSSH version",
                "software",
            ),
        ] {
            if let Some(version) = get_string(session, oid).await? {
                device.add_firmware(Firmware {
                    name: Some(format!("{model} {suffix}")),
                    description: Some(description.to_owned()),
                    r#type: Some(kind.to_owned()),
                    version: Some(version),
                    manufacturer: Some("Avaya".to_owned()),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AvayaMib;
    use crate::snmp::mib::{DeviceInfo, MibSupport, NetworkDevice};
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_avaya_sip_endpoints() {
        assert!(AvayaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.6889.1.69.6.1".to_owned()),
            ..DeviceInfo::default()
        }));
        assert!(!AvayaMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.6889.1.69.5".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn fills_identity_and_component_firmwares() {
        let mut session = WalkSession::parse(
            ".1.3.6.1.4.1.6889.2.69.6.1.4.0 = STRING: \"J169_SIP_R4.0.7\"\n\
             .1.3.6.1.4.1.6889.2.69.6.1.27.0 = STRING: \"1.2.3\"\n\
             .1.3.6.1.4.1.6889.2.69.6.1.52.0 = STRING: \"J169\"\n\
             .1.3.6.1.4.1.6889.2.69.6.1.57.0 = STRING: \"19AVST123456\"\n\
             .1.3.6.1.4.1.6889.2.69.6.1.139.0 = STRING: \"4\"\n\
             .1.3.6.1.4.1.6889.2.69.6.1.168.0 = STRING: \"1.1.1k\"\n",
        )
        .unwrap();
        let mut device = NetworkDevice::default();
        AvayaMib.run(&mut session, &mut device).await.unwrap();
        assert_eq!(device.info.r#type.as_deref(), Some("NETWORKING"));
        assert_eq!(device.info.firmware.as_deref(), Some("J169_SIP_R4.0.7"));
        assert_eq!(device.info.model.as_deref(), Some("J169"));
        assert_eq!(device.info.serial.as_deref(), Some("19AVST123456"));
        // DSP, hardware and OpenSSL present; OpenSSH absent.
        assert_eq!(device.firmwares.len(), 3);
        assert_eq!(
            device.firmwares[0].name.as_deref(),
            Some("J169 DSP firmware")
        );
        assert_eq!(device.firmwares[2].name.as_deref(), Some("J169 OpenSSL"));
    }
}
