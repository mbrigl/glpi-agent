// SPDX-License-Identifier: GPL-2.0-only

package discovery

// Fifth batch of upstream SNMP/MibSupport/* vendor modules (console servers /
// blades / storage / UPS), ported verbatim from the Perl OIDs.

func init() {
	// --- Avocent (console servers) ---
	const acsAppliance = "1.3.6.1.4.1.10418.26.2.1"
	registerMib(MibModule{
		Name:         "avocent",
		SysObjectID:  oidMatch("1.3.6.1.4.1.10418"),
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, acsAppliance+".2.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, acsAppliance+".4.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, acsAppliance+".7.0") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, acsAppliance+".1.0") },
	})

	// --- Bachmann (IPM PDUs) ---
	const e3Ipm = "1.3.6.1.4.1.21695.1.10.7"
	registerMib(MibModule{
		Name:         "bachmann",
		SysObjectID:  oidMatch("1.3.6.1.4.1.21695.1"),
		Manufacturer: func(SNMPGetter, Device) string { return "Bachmann" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, e3Ipm+".1.1") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, e3Ipm+".1.4") },
	})

	// --- Cisco UCS blade, priority 5 ---
	const cucsBoard = "1.3.6.1.4.1.9.9.719.1.9.6.1"
	registerMib(MibModule{
		Name:       "cisco-ucs-board",
		Priority:   5,
		PrivateOID: cucsBoard + ".2.1", // cucsComputeBoardDn
		Model:      func(g SNMPGetter, _ Device) string { return mibGet(g, cucsBoard+".6.1") },
		Serial:     func(g SNMPGetter, _ Device) string { return mibGet(g, cucsBoard+".14.1") },
	})

	// --- Radware DefensePro ---
	const defencepro = "1.3.6.1.4.1.89"
	registerMib(MibModule{
		Name:        "defencepro",
		SysObjectID: oidMatch(defencepro),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, defencepro+".2.14.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, defencepro+".2.12.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, defencepro+".35.1.34") },
		Mac:         func(g SNMPGetter, _ Device) string { return mibGet(g, defencepro+".35.1.69.5.0") },
	})

	// --- DigiPower (PDUs) ---
	const digipower = "1.3.6.1.4.1.17420"
	registerMib(MibModule{
		Name:        "digipower",
		SysObjectID: oidMatch(digipower),
		Type:        func(SNMPGetter, Device) string { return "NETWORKING" },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, digipower+".1.2.4.0") },
		Mac:         func(g SNMPGetter, _ Device) string { return mibGet(g, digipower+".1.2.3.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, digipower+".1.2.9.1.19.0") },
	})

	// --- FoxGate (switches) ---
	const foxgateOS = "1.3.6.1.4.1.6339.100"
	registerMib(MibModule{
		Name:        "foxgate",
		SysObjectID: oidMatch("1.3.6.1.4.1.6339"),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, foxgateOS+".1.3.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, foxgateOS+".25.1.1.1.0") },
	})

	// --- Hitachi Vantara (storage) ---
	registerMib(MibModule{
		Name:         "hitachi-vantara",
		SysObjectID:  oidMatch("1.3.6.1.4.1.116.3.11.4.1.1"),
		Type:         func(SNMPGetter, Device) string { return "STORAGE" },
		Manufacturer: func(SNMPGetter, Device) string { return "Hitachi Vantara" },
		Serial:       func(g SNMPGetter, _ Device) string { return walkFirst(g, "1.3.6.1.4.1.116.5.11.4.1.1.5.1") },
	})

	// --- HP HTTP management (ProCurve) ---
	const hpHttpMg = "1.3.6.1.4.1.11.2.36.1.1.2"
	registerMib(MibModule{
		Name:        "hp-http-management",
		SysObjectID: oidMatch("1.3.6.1.4.1.11.2.3.7.11"),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, hpHttpMg+".8.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, hpHttpMg+".9.0") },
	})

	// --- Infortrend (storage) ---
	const ifInfo = "1.3.6.1.4.1.1714.1.1.1.1"
	registerMib(MibModule{
		Name:        "infortrend",
		SysObjectID: oidMatch("1.3.6.1.4.1.1714.1.1"),
		Type:        func(SNMPGetter, Device) string { return "STORAGE" },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, ifInfo+".10.0") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, ifInfo+".15.0") },
		Firmware: func(g SNMPGetter, _ Device) string {
			maj, min := mibGet(g, ifInfo+".4.0"), mibGet(g, ifInfo+".5.0")
			if maj == "" {
				return ""
			}
			return maj + "." + min
		},
	})

	// --- Multitech (cellular routers) ---
	const mtsRouter = "1.3.6.1.4.1.995.15.1.1"
	registerMib(MibModule{
		Name:       "multitech",
		PrivateOID: mtsRouter + ".1.0",
		Type:       func(SNMPGetter, Device) string { return "NETWORKING" },
		Serial:     func(g SNMPGetter, _ Device) string { return mibGet(g, mtsRouter+".2.0") },
		Model:      func(g SNMPGetter, _ Device) string { return mibGet(g, mtsRouter+".1.0") },
		Firmware:   func(g SNMPGetter, _ Device) string { return mibGet(g, mtsRouter+".3.0") },
	})

	// --- Quantum (tape / storage) ---
	const quantumInfo = "1.3.6.1.4.1.3764.1.1.10"
	registerMib(MibModule{
		Name:         "quantum",
		SysObjectID:  oidMatch("1.3.6.1.4.1.3764"),
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, quantumInfo+".2.0") },
		Manufacturer: func(g SNMPGetter, _ Device) string { return mibGet(g, quantumInfo+".6.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, quantumInfo+".3.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, quantumInfo+".10.0") },
	})

	// --- Radware Alteon (load balancers) ---
	const radwareHw = "1.3.6.1.4.1.1872.2.5.1.3.1"
	registerMib(MibModule{
		Name:         "radware",
		SysObjectID:  oidMatch("1.3.6.1.4.1.1872"),
		Manufacturer: func(SNMPGetter, Device) string { return "Radware" },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, "1.3.6.1.4.1.1872.2.5.1.1.1.77.0") },
		Mac:          func(g SNMPGetter, _ Device) string { return mibGet(g, radwareHw+".13.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, radwareHw+".18.0") },
	})

	// --- UPS: standard UPS-MIB, APC and Riello ---
	const upsMIB = "1.3.6.1.2.1.33.1.1"
	registerMib(MibModule{
		Name:         "ups-std",
		SysObjectID:  oidMatch("1.3.6.1.2.1.33"),
		Manufacturer: func(g SNMPGetter, _ Device) string { return mibGet(g, upsMIB+".1.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, upsMIB+".2.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, upsMIB+".3.0") },
	})
	const apc = "1.3.6.1.4.1.318"
	registerMib(MibModule{
		Name:        "ups-apc",
		SysObjectID: oidMatch(apc),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, apc+".1.1.4.1.4.0") },
		Serial: func(g SNMPGetter, _ Device) string {
			return firstNonEmpty(mibGet(g, apc+".1.1.1.1.2.3.0"), mibGet(g, apc+".1.1.4.1.5.0"))
		},
		Firmware: func(g SNMPGetter, _ Device) string { return mibGet(g, apc+".1.1.4.1.2.0") },
	})
	const riello = "1.3.6.1.4.1.5491"
	registerMib(MibModule{
		Name:         "ups-riello",
		SysObjectID:  oidMatch(riello),
		Manufacturer: func(g SNMPGetter, _ Device) string { return mibGet(g, riello+".10.1.1.1.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, riello+".10.1.1.2.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, riello+".10.1.1.3.0") },
	})

	// --- Voltronic (UPS) ---
	const voltronicIdent = "1.3.6.1.4.1.43943.1.1.1"
	registerMib(MibModule{
		Name:        "voltronic",
		SysObjectID: oidMatch("1.3.6.1.4.1.43943"),
		Type:        func(SNMPGetter, Device) string { return "NETWORKING" },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, voltronicIdent+".3.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, voltronicIdent+".4.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, voltronicIdent+".6.0") },
		Manufacturer: func(g SNMPGetter, _ Device) string {
			return firstNonEmpty(mibGet(g, voltronicIdent+".1.0"), "Voltronic")
		},
	})
}
