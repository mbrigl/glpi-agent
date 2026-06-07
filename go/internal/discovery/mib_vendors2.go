// SPDX-License-Identifier: GPL-2.0-only

package discovery

// Second batch of upstream SNMP/MibSupport/* vendor modules, ported verbatim
// from the Perl OIDs. Printer modules' PAGECOUNTERS/CARTRIDGES sections (which
// need component-list support) are follow-on; the identity accessors are ported
// here.

func init() {
	// --- Cisco (priority 5) ---
	const entPhysicalModelName = "1.3.6.1.2.1.47.1.1.1.1.13"
	const cisco = "1.3.6.1.4.1.9"
	registerMib(MibModule{
		Name:         "cisco",
		Priority:     5,
		SysObjectID:  oidMatch(cisco),
		Model:        func(g SNMPGetter, _ Device) string { return walkFirst(g, entPhysicalModelName) },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, cisco+".2.1.3.0") },
	})

	// --- Juniper (first chassis member) ---
	const juniperMIB = "1.3.6.1.4.1.2636"
	const jnxVC = juniperMIB + ".3.40.1.4.1.1.1"
	registerMib(MibModule{
		Name:        "juniper",
		SysObjectID: oidMatch(juniperMIB),
		Serial:      func(g SNMPGetter, _ Device) string { return walkFirst(g, jnxVC+".2") },
		Mac:         func(g SNMPGetter, _ Device) string { return walkFirst(g, jnxVC+".4") },
		Firmware:    func(g SNMPGetter, _ Device) string { return walkFirst(g, jnxVC+".5") },
		Model:       func(g SNMPGetter, _ Device) string { return walkFirst(g, jnxVC+".8") },
	})

	// --- HP network peripheral (printer), priority 9 ---
	const hpPeripheral = "1.3.6.1.4.1.11.2.3.9"
	const hpSystemId = hpPeripheral + ".4.2.1.1.3"
	registerMib(MibModule{
		Name:        "hp-peripheral",
		Priority:    9,
		SysObjectID: oidMatch(hpPeripheral),
		PrivateOID:  hpPeripheral + ".1.1.7.0", // gdStatusId
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, hpSystemId+".2.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, hpSystemId+".3.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, hpSystemId+".6.0") },
	})

	// --- Brother (priority 5) ---
	const brother = "1.3.6.1.4.1.2435"
	const brconfig = brother + ".2.4.3.1240.1"
	const brInfoSerialNumber = brother + ".2.3.9.4.2.1.5.5.1.0"
	registerMib(MibModule{
		Name:         "brother-netconfig",
		Priority:     5,
		PrivateOID:   brconfig + ".3.0", // brpsHardwareType
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, brconfig+".3.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, brconfig+".4.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, brInfoSerialNumber) },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, brconfig+".1.0") },
	})

	// --- Canon (printer) ---
	const canon = "1.3.6.1.4.1.1602"
	const canProductInfo = canon + ".1.1.1"
	registerMib(MibModule{
		Name:        "canon",
		SysObjectID: oidMatch(canon + ".4"),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, canProductInfo+".1.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, canProductInfo+".4.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, canon+".1.2.1.8.1.3.1.1") },
	})

	// --- Epson (printer) ---
	const epson = "1.3.6.1.4.1.1248"
	registerMib(MibModule{
		Name:        "epson-printer",
		SysObjectID: oidMatch(epson),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, epson+".1.2.2.1.1.1.5.1") },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, epson+".1.2.2.1.1.1.2.1") },
	})

	// --- Konica / Sindoh (printer) ---
	const konica = "1.3.6.1.4.1.18334"
	registerMib(MibModule{
		Name:        "konica-printer",
		SysObjectID: oidMatch(konica + ".1.1.1.2"),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, konica+".1.1.1.1.6.2.1.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, konica+".1.1.1.5.5.1.1.3") },
	})

	// --- Ricoh (printer) ---
	const ricohAgentsID = "1.3.6.1.4.1.367.1.1"
	registerMib(MibModule{
		Name:         "ricoh-printer",
		SysObjectID:  oidMatch(ricohAgentsID),
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, "1.3.6.1.2.1.43.5.1.1.16.1") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, "1.3.6.1.4.1.367.3.2.1.6.1.1.7.1") },
	})

	// --- Kyocera (printer), priority 7 ---
	const kyocera = "1.3.6.1.4.1.1347"
	registerMib(MibModule{
		Name:         "kyocera",
		Priority:     7,
		SysObjectID:  oidMatch(kyocera + ".41"),
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, kyocera+".40.10.1.1.5.1") },
	})

	// --- Lexmark (printer) ---
	const lexmark = "1.3.6.1.4.1.641"
	const prtgenInfo = lexmark + ".2.1.2.1"
	registerMib(MibModule{
		Name:        "lexmark",
		SysObjectID: oidMatch(lexmark),
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, prtgenInfo+".2.1") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, prtgenInfo+".4.1") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, prtgenInfo+".6.1") },
	})

	// --- Zebra (label printers), priority 7: esi + zebra arcs ---
	const esi = "1.3.6.1.4.1.683"
	registerMib(MibModule{
		Name:        "zebra-printer",
		Priority:    7,
		SysObjectID: oidMatch(esi),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, esi+".1.5.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, esi+".1.9.0") },
	})
	const zebra = "1.3.6.1.4.1.10642"
	const zbrGeneralInfo = zebra + ".1"
	registerMib(MibModule{
		Name:         "zebra-printer-zt",
		Priority:     7,
		SysObjectID:  oidMatch(zbrGeneralInfo + ".1"),
		Model:        func(g SNMPGetter, _ Device) string { return mibGet(g, zbrGeneralInfo+".1.0") },
		Firmware:     func(g SNMPGetter, _ Device) string { return mibGet(g, zbrGeneralInfo+".2.0") },
		Serial:       func(g SNMPGetter, _ Device) string { return mibGet(g, zbrGeneralInfo+".9.0") },
		SnmpHostname: func(g SNMPGetter, _ Device) string { return mibGet(g, zbrGeneralInfo+".4.0") },
	})
}
