// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "strings"

// Third batch of upstream SNMP/MibSupport/* vendor modules (network / security /
// storage / server management), ported verbatim from the Perl OIDs.

func init() {
	// --- Aruba (Instant AP) ---
	const aruba = "1.3.6.1.4.1.14823"
	const aiMIB = aruba + ".2.3.3.1"
	registerMib(MibModule{
		Name:        "aruba",
		SysObjectID: oidMatch(aruba),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, aiMIB+".1.4.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return walkFirst(g, aiMIB+".2.1.1.4") },
		Model:       func(g SNMPGetter, _ Device) string { return walkFirst(g, aiMIB+".2.1.1.6") },
	})

	// --- Avaya (J100 IP phones) ---
	const avaya = "1.3.6.1.4.1.6889"
	const avayaEndpt = avaya + ".2.69.6.1" // endptID
	registerMib(MibModule{
		Name:        "avaya-j100-ipphone",
		SysObjectID: oidMatch(avaya + ".1.69.6"),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, avayaEndpt+".52.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, avayaEndpt+".4.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, avayaEndpt+".57.0") },
	})

	// --- Brocade (FibreChannel / IP switches) ---
	const brocade = "1.3.6.1.4.1.1991"
	registerMib(MibModule{
		Name:        "brocade-switch",
		SysObjectID: oidMatch(brocade),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, brocade+".1.1.1.1.2.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, brocade+".1.1.2.1.11.0") },
	})

	// --- Check Point (SVN appliances) ---
	const checkpoint = "1.3.6.1.4.1.2620"
	const svnInfo = checkpoint + ".1.6.4"
	const svnAppliance = checkpoint + ".1.6.16"
	registerMib(MibModule{
		Name:         "CheckPoint",
		SysObjectID:  oidMatch(checkpoint),
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, svnAppliance+".3.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, svnAppliance+".7.0") },
		Manufacturer: func(g SNMPGetter, _ Device) string { return mibGet(g, svnAppliance+".9.0") },
		Firmware: func(g SNMPGetter, _ Device) string {
			ver := mibGet(g, svnInfo+".1.0")
			if ver == "" {
				return ""
			}
			if build := mibGet(g, svnInfo+".2.0"); build != "" {
				return ver + " (build " + build + ")"
			}
			return ver
		},
	})

	// --- D-Link (relative to the device's own sysObjectID) ---
	const dlinkProducts = "1.3.6.1.4.1.171.10"
	dlinkPriv := func(g SNMPGetter, d Device, suboid string) string {
		sysoid, _ := d["SYSOBJECTID"].(string)
		if sysoid == "" {
			return ""
		}
		return getOne(g, strings.TrimPrefix(sysoid, ".")+suboid)
	}
	registerMib(MibModule{
		Name:         "d-link",
		SysObjectID:  oidMatch(dlinkProducts),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "D-Link" },
		Firmware:     func(g SNMPGetter, d Device) string { return dlinkPriv(g, d, ".1.3.0") },
		Serial:       func(g SNMPGetter, d Device) string { return dlinkPriv(g, d, ".1.18.0") },
	})

	// --- Hikvision (cameras / NVRs) ---
	// See Hikvision-MIB: identity under .39165 plus a second entity tree .50001.
	const hikvision = "1.3.6.1.4.1.39165"
	const hikvisionModel = hikvision + ".1.1.0"
	const hikvisionMac = hikvision + ".1.4.0"
	const hikvision2 = "1.3.6.1.4.1.50001"
	const hikEntityIndex = hikvision2 + ".1.3.0"
	// getSerial: the entity index if present, else the MAC with dashes stripped.
	hikSerial := func(g SNMPGetter, _ Device) string {
		if idx := mibGet(g, hikEntityIndex); idx != "" {
			return idx
		}
		mac := mibGet(g, hikvisionMac)
		if mac == "" {
			return ""
		}
		return strings.ReplaceAll(mac, "-", "")
	}
	hik := MibModule{
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Hikvision" },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, hikvisionModel) },
		Serial:       hikSerial,
		Mac: func(g SNMPGetter, _ Device) string {
			mac := mibGet(g, hikvisionMac)
			if mac == "" {
				return ""
			}
			return canonicalMAC(mac)
		},
		// getSnmpHostname: MODEL_serial once MODEL is known (set earlier in the field loop).
		SnmpHostname: func(g SNMPGetter, d Device) string {
			serial := hikSerial(g, d)
			model, _ := d["MODEL"].(string)
			if serial == "" || strings.TrimSpace(model) == "" {
				return ""
			}
			return model + "_" + serial
		},
	}
	hik.Name, hik.SysObjectID = "hikvision", oidMatch(hikvision)
	registerMib(hik)
	hik.Name, hik.SysObjectID = "hikvision-50001", oidMatch(hikvision2)
	registerMib(hik)
	hik.Name, hik.SysObjectID, hik.PrivateOID = "hikvision-model", nil, hikvisionModel
	registerMib(hik)

	// --- HP iLO (Integrated Lights-Out, CPQSM2) ---
	const cpqSm2Cntrl = "1.3.6.1.4.1.232.9.2.2"
	registerMib(MibModule{
		Name:        "cpqsm2",
		SysObjectID: oidMatch("1.3.6.1.4.1.232.9.4"),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, cpqSm2Cntrl+".2.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, cpqSm2Cntrl+".15.0") },
	})

	// --- Dell iDRAC ---
	const idrac = "1.3.6.1.4.1.674.10892"
	registerMib(MibModule{
		Name:        "idrac",
		SysObjectID: oidMatch(idrac),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, idrac+".2.1.1.11.0") },
	})

	// --- Nokia / Alcatel (Timetra SR) ---
	const timetra = "1.3.6.1.4.1.6527"
	const tmnxHw = timetra + ".3.1.2.2.1.8.1"
	registerMib(MibModule{
		Name:        "nokia",
		SysObjectID: oidMatch(timetra),
		Serial:      func(g SNMPGetter, _ Device) string { return walkFirst(g, tmnxHw+".5") },
		Model:       func(g SNMPGetter, _ Device) string { return walkFirst(g, tmnxHw+".8") },
		Firmware:    func(g SNMPGetter, _ Device) string { return walkFirst(g, tmnxHw+".21") },
	})

	// --- SonicWall (firewalls) ---
	const snwlSys = "1.3.6.1.4.1.8741.2.1.1"
	registerMib(MibModule{
		Name:        "sonicwall",
		SysObjectID: oidMatch("1.3.6.1.4.1.8741"),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, snwlSys+".1.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, snwlSys+".2.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, snwlSys+".3.0") },
	})

	// --- Sophos (XG firewalls) ---
	const sfosXGDeviceInfo = "1.3.6.1.4.1.2604.5.1.1"
	registerMib(MibModule{
		Name:        "sophos",
		SysObjectID: oidMatch("1.3.6.1.4.1.2604.5"),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, sfosXGDeviceInfo+".2.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, sfosXGDeviceInfo+".3.0") },
	})

	// --- TP-Link (managed switches / EAP) ---
	const tplinkSysInfo = "1.3.6.1.4.1.11863.6.1.1.1"
	registerMib(MibModule{
		Name:        "tplink",
		SysObjectID: oidMatch("1.3.6.1.4.1.11863"),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, tplinkSysInfo+".6.0") },
		Mac:         func(g SNMPGetter, _ Device) string { return mibGet(g, tplinkSysInfo+".7.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, tplinkSysInfo+".8.0") },
	})

	// --- Zyxel (enterprise switches), priority 5 ---
	const esSysInfo = "1.3.6.1.4.1.890.1.15.3.1"
	registerMib(MibModule{
		Name:        "zyxel",
		Priority:    5,
		SysObjectID: oidMatch("1.3.6.1.4.1.890.1.15"),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, esSysInfo+".6.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, esSysInfo+".11.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, esSysInfo+".12.0") },
	})

	// --- OKI (printers) ---
	const oki = "1.3.6.1.4.1.2001"
	registerMib(MibModule{
		Name:        "oki",
		SysObjectID: oidMatch(oki),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, oki+".1.1.1.1.11.1.10.45.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, oki+".1.1.1.1.11.1.10.25.0") },
	})
}
