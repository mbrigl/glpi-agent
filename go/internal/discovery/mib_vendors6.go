// SPDX-License-Identifier: GPL-2.0-only

package discovery

// Sixth (final mainstream) batch of upstream SNMP/MibSupport/* vendor modules,
// ported verbatim from the Perl OIDs. A few modules with heavy index/conditional
// logic (EMC, Panasas, Siemens, LinuxAppliance, FreeBSD/Stormshield) and the two
// SNMP-framework infra modules remain follow-on.

func init() {
	// --- Aerohive (WLAN) ---
	const ahSystem = "1.3.6.1.4.1.26928.1.2"
	registerMib(MibModule{
		Name:         "aerohive",
		SysObjectID:  oidMatch("1.3.6.1.4.1.26928"),
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, ahSystem+".5.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, ahSystem+".12.0") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, ahSystem+".1.0") },
	})

	// --- AKCP (sensor probes) ---
	const akcp = "1.3.6.1.4.1.3854"
	const akcpCfg = akcp + ".3.2.1"
	registerMib(MibModule{
		Name:         "akcp",
		SysObjectID:  oidMatch(akcp),
		Mac:          func(g SNMPGetter, _ Device) string { return mibGet(g, akcp+".1.2.2.1.3.0") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, akcpCfg+".9.0") },
	})

	// --- Citrix NetScaler ---
	const netScaler = "1.3.6.1.4.1.5951"
	registerMib(MibModule{
		Name:        "netscaler",
		SysObjectID: oidMatch(netScaler),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, netScaler+".4.1.1.14.0") },
	})

	// --- D-Link DGS-1210 series ---
	const dgsCommon = "1.3.6.1.4.1.171.11.153.1000.1"
	registerMib(MibModule{
		Name:         "dlink-dgs1210",
		SysObjectID:  oidMatch("1.3.6.1.4.1.171.10.153"),
		Manufacturer: func(SNMPGetter, Device) string { return "D-Link" },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, dgsCommon+".1.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, dgsCommon+".3.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, dgsCommon+".33.1.0") },
	})

	// --- Digi (Sarian / TransPort routers) ---
	const sarianSystem = "1.3.6.1.4.1.16378.10000.3"
	registerMib(MibModule{
		Name:        "digi",
		SysObjectID: oidMatch("1.3.6.1.4.1.16378.10000"),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, sarianSystem+".16.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, sarianSystem+".15.0") },
	})

	// --- HP Citizen (storage management) ---
	const hpCitizenMg = "1.3.6.1.4.1.11.2.36.1.1.2"
	registerMib(MibModule{
		Name:         "hp-citizen",
		SysObjectID:  oidMatch("1.3.6.1.4.1.11.10"),
		Type:         func(SNMPGetter, Device) string { return "STORAGE" },
		Manufacturer: func(g SNMPGetter, _ Device) string { return mibGet(g, hpCitizenMg+".4.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, hpCitizenMg+".5.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, hpCitizenMg+".6.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, hpCitizenMg+".9.0") },
	})

	// --- RNX (PDUs) ---
	const upduMib2 = "1.3.6.1.4.1.55108.2"
	registerMib(MibModule{
		Name:         "rnx",
		SysObjectID:  oidMatch("1.3.6.1.4.1.55108"),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "RNX" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, upduMib2+".1.2.1.5.1") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, upduMib2+".6.2.1.9.1") },
	})

	// --- Telco Systems (switches) ---
	const prvtSwitch = "1.3.6.1.4.1.738.1.5.100.1.3"
	registerMib(MibModule{
		Name:         "telco",
		SysObjectID:  oidMatch("1.3.6.1.4.1.738"),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Telco Systems" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, prvtSwitch+".1.0") },
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, prvtSwitch+".2.0") },
	})

	// --- Tiesse (routers) ---
	const tiesse = "1.3.6.1.4.1.4799"
	registerMib(MibModule{
		Name:         "tiesse",
		SysObjectID:  oidMatch(tiesse),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Tiesse" },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, tiesse+".200.1.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, tiesse+".200.2.0") },
		Model: func(g SNMPGetter, _ Device) string {
			return firstNonEmpty(mibGet(g, tiesse+".3.2.6023.0"), mibGet(g, "1.3.6.1.2.1.47.1.1.1.1.2.0"))
		},
	})

	// --- Voltaire (InfiniBand) ---
	const voltaire = "1.3.6.1.4.1.5206"
	registerMib(MibModule{
		Name:         "voltaire",
		SysObjectID:  oidMatch(voltaire),
		Type:         func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(SNMPGetter, Device) string { return "Voltaire" },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, voltaire+".3.29.1.3.1007.1") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, voltaire+".3.1.0") },
	})
}
