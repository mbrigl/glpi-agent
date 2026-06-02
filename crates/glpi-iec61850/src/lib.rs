// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-iec61850` — IEC 61850 (MMS) device inventory.
//!
//! Ported from the upstream `GLPI::Agent::IEC61850::{Protocol,Device}`: scan an
//! Intelligent Electronic Device (IED) for its physical-nameplate identity and
//! build the GLPI inventory (INFO / ITEMTYPE / FIRMWARES, including the GLPI 11+
//! `IedAsset` itemtype).
//!
//! # Transport boundary
//!
//! The upstream agent performs the on-wire MMS exchange through **libiec61850**
//! (a C library, bound via SWIG). The library-independent scan/inventory logic
//! and the [`IedProtocol`] seam it runs over are implemented and tested here
//! against the in-memory [`MockProtocol`]. A real backend implements
//! [`IedProtocol`]:
//!
//! - the **libiec61850 FFI** backend (behind the off-by-default `libiec61850`
//!   feature) needs the C library plus a C toolchain/bindgen at build time, or
//! - a pure-Rust MMS client.
//!
//! Neither on-wire backend is built by default, so the crate compiles and tests
//! anywhere; only the transport plug-in is environment-dependent.

pub mod device;
pub mod mock;
pub mod protocol;

pub use device::{IedFirmware, IedIdentity, IedInfo, IedInventory, IedIps};
pub use mock::MockProtocol;
pub use protocol::{FunctionalConstraint, IedProtocol};

#[cfg(test)]
mod tests {
    use crate::{IedIdentity, MockProtocol};

    /// A Siemens-style IED: one logical device with an `LPHD1` physical node
    /// carrying a full `PhyNam`, plus a distracting logical node and data object
    /// to prove the scan picks the right ones.
    fn siemens_ied() -> MockProtocol {
        let ld = "IED1A_Allg";
        MockProtocol::new()
            .with_logical_device(ld, &["LLN0", "LPHD1", "CSWI1"])
            .with_data_objects(&format!("{ld}/LPHD1"), &["Proxy", "PhyNam", "PhyHealth"])
            .with_value(&format!("{ld}/LPHD1.PhyNam.vendor"), "SIEMENS")
            .with_value(&format!("{ld}/LPHD1.PhyNam.model"), "7SJ8221")
            .with_value(&format!("{ld}/LPHD1.PhyNam.serNum"), "BF1234567")
            .with_value(&format!("{ld}/LPHD1.PhyNam.swRev"), "V07.80")
            .with_value(&format!("{ld}/LPHD1.PhyNam.hwRev"), "EE")
            .with_value(&format!("{ld}/LPHD1.PhyNam.owner"), "Substation A")
            .with_value(&format!("{ld}/LPHD1.PhyNam.location"), "Bay 3")
    }

    #[tokio::test]
    async fn scans_physical_nameplate() {
        let mut ied = siemens_ied();
        let identity = IedIdentity::scan(&mut ied).await.unwrap();
        assert_eq!(identity.ied_name.as_deref(), Some("IED1A_Allg"));
        assert_eq!(identity.manufacturer.as_deref(), Some("SIEMENS"));
        assert_eq!(identity.model.as_deref(), Some("7SJ8221"));
        assert_eq!(identity.serial.as_deref(), Some("BF1234567"));
        assert_eq!(identity.firmware.as_deref(), Some("V07.80"));
        assert_eq!(identity.hardware.as_deref(), Some("EE"));
        assert_eq!(identity.contact.as_deref(), Some("Substation A"));
        assert_eq!(identity.location.as_deref(), Some("Bay 3"));
    }

    #[tokio::test]
    async fn builds_glpi11_inventory_with_cleaned_name() {
        let mut ied = siemens_ied();
        let identity = IedIdentity::scan(&mut ied).await.unwrap();
        let inventory = identity.into_inventory(
            Some("11.0.1"),
            Some("00:0c:29:aa:bb:cc".to_owned()),
            Some("10.0.0.42".to_owned()),
        );

        // GLPI 11 -> IED custom asset.
        assert_eq!(
            inventory.itemtype.as_deref(),
            Some(r"Glpi\CustomAsset\IedAsset")
        );
        // The `A_Allg` suffix is stripped from the IED name.
        assert_eq!(inventory.info.name.as_deref(), Some("IED1"));
        assert_eq!(inventory.info.r#type, "NETWORKING");
        assert_eq!(inventory.info.manufacturer.as_deref(), Some("SIEMENS"));
        assert_eq!(inventory.info.mac.as_deref(), Some("00:0c:29:aa:bb:cc"));
        assert_eq!(inventory.info.ips.as_ref().unwrap().ip, "10.0.0.42");

        // Firmware + hardware entries.
        assert_eq!(inventory.firmwares.len(), 2);
        assert_eq!(inventory.firmwares[0].name, "7SJ8221 firmware");
        assert_eq!(inventory.firmwares[0].version.as_deref(), Some("V07.80"));
        assert_eq!(inventory.firmwares[1].name, "7SJ8221 hardware");
        assert_eq!(inventory.firmwares[1].version.as_deref(), Some("EE"));

        // Serializes to the GLPI native uppercase keys.
        let json = serde_json::to_value(&inventory).unwrap();
        assert_eq!(json["INFO"]["MODEL"], "7SJ8221");
        assert_eq!(json["INFO"]["IPS"]["IP"], "10.0.0.42");
        assert_eq!(json["FIRMWARES"][0]["TYPE"], "ied");
    }

    #[tokio::test]
    async fn older_glpi_has_no_itemtype_and_no_hardware_entry() {
        let mut ied = MockProtocol::new()
            .with_logical_device("IED2", &["LPHD1"])
            .with_data_objects("IED2/LPHD1", &["PhyNam"])
            .with_value("IED2/LPHD1.PhyNam.vendor", "ABB")
            .with_value("IED2/LPHD1.PhyNam.swRev", "1.2");
        let inventory =
            IedIdentity::scan(&mut ied)
                .await
                .unwrap()
                .into_inventory(Some("10.0.19"), None, None);
        assert!(inventory.itemtype.is_none());
        // No model -> generic firmware name; no hwRev -> single firmware entry.
        assert_eq!(inventory.firmwares.len(), 1);
        assert_eq!(inventory.firmwares[0].name, "Electronic device firmware");
        assert_eq!(inventory.info.name.as_deref(), Some("IED2"));
    }

    #[tokio::test]
    async fn empty_server_yields_bare_identity() {
        let mut ied = MockProtocol::new();
        let identity = IedIdentity::scan(&mut ied).await.unwrap();
        assert_eq!(identity, IedIdentity::default());
    }
}
