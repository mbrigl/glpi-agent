// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"regexp"
	"strings"
)

// Fourth batch of upstream SNMP/MibSupport/* vendor modules (storage / PDUs /
// VoIP / appliances / printers), ported verbatim from the Perl OIDs.

func init() {
	// --- QNAP (NAS), priority 5 ---
	const qnap = "1.3.6.1.4.1.24681"
	const qnapSysInfo = qnap + ".2.2.2"
	registerMib(MibModule{
		Name:         "qnap",
		Priority:     5,
		SysObjectID:  oidMatch(qnap),
		PrivateOID:   qnapSysInfo + ".12.0",
		Type:         func(SNMPGetter, Device) string { return "STORAGE" },
		Manufacturer: func(SNMPGetter, Device) string { return "Qnap" },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, qnapSysInfo+".12.0") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, qnapSysInfo+".13.0") },
	})

	// --- Ruckus (WLAN) ---
	const ruckus = "1.3.6.1.4.1.25053"
	const ruckusHwInfo = ruckus + ".1.1.2.1.1.1"
	const ruckusSwInfo = ruckus + ".1.1.3.1.1.1"
	registerMib(MibModule{
		Name:        "ruckus",
		SysObjectID: oidMatch(ruckus + ".3"),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, ruckusHwInfo+".1.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, ruckusHwInfo+".2.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, ruckusSwInfo+".1.1.3.1") },
	})

	// --- Cisco Meraki ---
	registerMib(MibModule{
		Name:         "cisco-meraki",
		SysObjectID:  oidMatch("1.3.6.1.4.1.29671.2"),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Cisco Meraki" },
	})

	// --- Eaton ePDU + Eaton/Powerware UPS (xups) ---
	const epdu = "1.3.6.1.4.1.534.6.6.7"
	registerMib(MibModule{
		Name:        "eaton-epdu",
		SysObjectID: oidMatch(epdu),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, epdu+".1.2.1.3.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, epdu+".1.2.1.4.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, epdu+".1.2.1.5.0") },
	})
	const xups = "1.3.6.1.4.1.534.1.1"
	registerMib(MibModule{
		Name:         "eaton-xups",
		SysObjectID:  oidMatch("1.3.6.1.4.1.534.1"),
		Manufacturer: func(g SNMPGetter, _ Device) string { return mibGet(g, xups+".1.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, xups+".2.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, xups+".3.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, xups+".6.0") },
	})

	// --- Raritan (PDU2) ---
	const nameplate = "1.3.6.1.4.1.13742.6.3.2.1.1.1"
	registerMib(MibModule{
		Name:         "raritan",
		SysObjectID:  oidMatch("1.3.6.1.4.1.13742.6"),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(g SNMPGetter, _ Device) string { return firstNonEmpty(mibGet(g, nameplate+".2.1"), "Raritan") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, nameplate+".3.1") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, nameplate+".4.1") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, "1.3.6.1.4.1.13742.6.3.2.2.1.13.1") },
	})

	// --- Snom (VoIP phones) ---
	const snomFirmware = "1.3.6.1.2.1.7526.2.4"
	registerMib(MibModule{
		Name:         "snom",
		PrivateOID:   snomFirmware,
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Snom" },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, snomFirmware) },
	})

	// --- Htek (VoIP phones) ---
	const htek = "1.3.6.1.4.1.38241"
	registerMib(MibModule{
		Name:        "htek",
		SysObjectID: oidMatch(htek),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, htek+".1.1.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, htek+".1.2.0") },
		Mac:         func(g SNMPGetter, _ Device) string { return mibGet(g, htek+".1.3.0") },
	})

	// --- WatchGuard (firewalls) ---
	const wgInfo = "1.3.6.1.4.1.3097.6"
	registerMib(MibModule{
		Name:         "watchguard",
		SysObjectID:  oidMatch("1.3.6.1.4.1.3097"),
		Manufacturer: func(SNMPGetter, Device) string { return "WatchGuard" },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, wgInfo+".3.1.0") },
	})

	// --- Dell Wyse ThinOS thin clients ---
	const thinClient = "1.3.6.1.4.1.714.1.2"
	registerMib(MibModule{
		Name:         "wyse-thinos",
		SysObjectID:  oidMatch(thinClient),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Dell" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, thinClient+".6.2.1.0") },
		Model: func(_ SNMPGetter, d Device) string {
			descr, _ := d["DESCRIPTION"].(string)
			if m := regexp.MustCompile(`^(\S+)`).FindStringSubmatch(descr); m != nil {
				return "Wyse " + m[1]
			}
			return ""
		},
	})

	// --- Intelbras / Dahua (cameras) ---
	const dahua = "1.3.6.1.4.1.1004849"
	const dahuaSysInfo = dahua + ".2.1"
	registerMib(MibModule{
		Name:         "intelbras",
		SysObjectID:  oidMatch(dahua),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Intelbras" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, dahuaSysInfo+".2.4.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, dahuaSysInfo+".1.1.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, dahuaSysInfo+".2.6.0") },
	})

	// --- Pantum (printers) ---
	const pantum = "1.3.6.1.4.1.40093"
	const pantumPrinter = pantum + ".1.1"
	registerMib(MibModule{
		Name:         "pantum",
		SysObjectID:  oidMatch(pantumPrinter),
		Manufacturer: func(SNMPGetter, Device) string { return "Pantum" },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, "1.3.6.1.2.1.43.5.1.1.16.1") },
		Serial: func(g SNMPGetter, _ Device) string {
			return firstNonEmpty(mibGet(g, pantumPrinter+".1.5"), mibGet(g, pantum+".6.1.2"), mibGet(g, pantum+".10.1.1.4"))
		},
	})

	// --- Toshiba TEC (barcode printers) ---
	const toshibatec = "1.3.6.1.4.1.1129"
	const bcpGeneral = toshibatec + ".1.2.1.1.1.1.1"
	const bcpDevice = toshibatec + ".1.2.1.1.1.2"
	registerMib(MibModule{
		Name:        "toshiba",
		SysObjectID: oidMatch(toshibatec),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, bcpGeneral+".1.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, bcpGeneral+".2.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, bcpDevice+".1.0") },
	})

	// --- HWg (environment sensors) ---
	const hwg = "1.3.6.1.4.1.21796"
	hwgMac := func(g SNMPGetter, _ Device) string {
		return firstNonEmpty(mibGet(g, hwg+".4.5.70.1.0"), mibGet(g, hwg+".4.1.70.1.0"), mibGet(g, hwg+".4.9.70.1.0"))
	}
	registerMib(MibModule{
		Name:         "hwg",
		SysObjectID:  oidMatch(hwg),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "HW group s.r.o" },
		Mac:          hwgMac,
		Serial:       func(g SNMPGetter, d Device) string { return strings.ReplaceAll(hwgMac(g, d), ":", "") },
	})

	// --- Meinberg (time servers), priority 20 ---
	const mbgLtNgInfo = "1.3.6.1.4.1.5597.30.0.0"
	registerMib(MibModule{
		Name:         "meinberg",
		Priority:     20,
		SysObjectID:  oidMatch("1.3.6.1.4.1.5597"),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Meinberg" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, mbgLtNgInfo+".3.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, mbgLtNgInfo+".2.0") },
	})
}
