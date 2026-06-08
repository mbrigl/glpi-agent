// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

func TestOidMatch(t *testing.T) {
	re := oidMatch("1.3.6.1.4.1.14988")
	if !re.MatchString(".1.3.6.1.4.1.14988.1.1.3") {
		t.Error("prefix should match")
	}
	if re.MatchString(".1.3.6.1.4.1.149") {
		t.Error("a shorter/different OID must not match")
	}
}

// TestMikrotikMibSupport drives the framework end to end via the fake getter:
// a Mikrotik sysObjectID selects the module, which overrides SERIAL/FIRMWARE and
// derives MODEL from the description.
func TestMikrotikMibSupport(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "RouterOS CCR2004-1G-12S+2XS",
			oidSysObjectID: ".1.3.6.1.4.1.14988.2",
			// Mikrotik MIB scalars.
			"1.3.6.1.4.1.14988.1.1.7.3.0": "HEX0123456789",
			"1.3.6.1.4.1.14988.1.1.7.4.0": "7.13.2",
		},
	}

	device, err := GetInventory("192.0.2.9", getter)
	if err != nil {
		t.Fatal(err)
	}
	if device["SERIAL"] != "HEX0123456789" {
		t.Errorf("SERIAL = %v, want the Mikrotik serial", device["SERIAL"])
	}
	if device["FIRMWARE"] != "7.13.2" {
		t.Errorf("FIRMWARE = %v, want 7.13.2", device["FIRMWARE"])
	}
	if device["MODEL"] != "CCR2004-1G-12S+2XS" {
		t.Errorf("MODEL = %v, want it parsed from sysDescr", device["MODEL"])
	}
}

// TestMatchMibModulesPriority checks sysObjectID vs sysORID matching.
func TestMatchMibModulesPriority(t *testing.T) {
	// Ubiquiti matches both by sysObjectID and by the sysORID oid rule.
	bySysobj := matchMibModules(".1.3.6.1.4.1.41112.1.6", map[string]bool{}, nil)
	if !containsModule(bySysobj, "ubnt") {
		t.Error("ubnt should match by sysObjectID")
	}
	byOrid := matchMibModules("", map[string]bool{"1.3.6.1.4.1.41112": true}, nil)
	if !containsModule(byOrid, "ubnt") {
		t.Error("ubnt should match by sysORID oid rule")
	}
	if len(matchMibModules(".1.2.3.4", map[string]bool{}, nil)) != 0 {
		t.Error("an unrelated sysObjectID must match nothing")
	}
}

// TestLexmarkMibSupport checks a sysObjectID-matched printer module.
func TestLexmarkMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                   "Lexmark MX522",
		oidSysObjectID:                ".1.3.6.1.4.1.641.2",
		"1.3.6.1.4.1.641.2.1.2.1.2.1": "Lexmark MX522adhe",
		"1.3.6.1.4.1.641.2.1.2.1.4.1": "LW80.PR.P241",
		"1.3.6.1.4.1.641.2.1.2.1.6.1": "5012ABC0001",
	}}
	device, _ := GetInventory("192.0.2.20", getter)
	if device["MODEL"] != "Lexmark MX522adhe" || device["FIRMWARE"] != "LW80.PR.P241" || device["SERIAL"] != "5012ABC0001" {
		t.Errorf("lexmark device = %v", device)
	}
}

// TestHpPrivateOidMatch checks the privateoid match path (the device responds to
// the HP gdStatusId even though its sysObjectID is generic).
func TestHpPrivateOidMatch(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                          "HP ETHERNET MULTI-ENVIRONMENT",
		oidSysObjectID:                       ".1.3.6.1.4.1.11.2.3.9.1",
		"1.3.6.1.4.1.11.2.3.9.1.1.7.0":       "READY",       // gdStatusId -> privateoid match
		"1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.2.0": "HP LaserJet", // model
		"1.3.6.1.4.1.11.2.3.9.4.2.1.1.3.3.0": "CNB1234567",  // serial
	}}
	device, _ := GetInventory("192.0.2.21", getter)
	if device["MODEL"] != "HP LaserJet" || device["SERIAL"] != "CNB1234567" {
		t.Errorf("hp device = %v", device)
	}
}

// TestSonicWallMibSupport checks a firewall vendor module (scalar OIDs).
func TestSonicWallMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                  "SonicWALL",
		oidSysObjectID:               ".1.3.6.1.4.1.8741.1",
		"1.3.6.1.4.1.8741.2.1.1.1.0": "TZ470",
		"1.3.6.1.4.1.8741.2.1.1.2.0": "18B1690ABC00",
		"1.3.6.1.4.1.8741.2.1.1.3.0": "7.0.1-5083",
	}}
	d, _ := GetInventory("192.0.2.30", getter)
	if d["MODEL"] != "TZ470" || d["SERIAL"] != "18B1690ABC00" || d["FIRMWARE"] != "7.0.1-5083" {
		t.Errorf("sonicwall = %v", d)
	}
}

// TestCheckPointFirmwareCompose checks the composed "version (build N)" firmware
// and the appliance manufacturer override.
func TestCheckPointFirmwareCompose(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                   "Check Point",
		oidSysObjectID:                ".1.3.6.1.4.1.2620.1.1",
		"1.3.6.1.4.1.2620.1.6.4.1.0":  "R81.20",
		"1.3.6.1.4.1.2620.1.6.4.2.0":  "631",
		"1.3.6.1.4.1.2620.1.6.16.3.0": "1809BX1234",
		"1.3.6.1.4.1.2620.1.6.16.9.0": "Check Point",
	}}
	d, _ := GetInventory("192.0.2.31", getter)
	if d["FIRMWARE"] != "R81.20 (build 631)" {
		t.Errorf("FIRMWARE = %v, want 'R81.20 (build 631)'", d["FIRMWARE"])
	}
	if d["SERIAL"] != "1809BX1234" || d["MANUFACTURER"] != "Check Point" {
		t.Errorf("checkpoint = %v", d)
	}
}

// TestQnapMibSupport checks a STORAGE module that matches by privateoid and sets
// a constant Type/Manufacturer.
func TestQnapMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                    "Linux NAS 5.10",
		oidSysObjectID:                 ".1.3.6.1.4.1.8072.3.2.10", // generic net-snmp
		"1.3.6.1.4.1.24681.2.2.2.12.0": "TS-873A",                  // es_ModelName -> privateoid match
		"1.3.6.1.4.1.24681.2.2.2.13.0": "nas01",
	}}
	d, _ := GetInventory("192.0.2.40", getter)
	if d["TYPE"] != "STORAGE" || d["MANUFACTURER"] != "Qnap" || d["MODEL"] != "TS-873A" {
		t.Errorf("qnap = %v", d)
	}
}

// TestEatonEpduMibSupport checks an ePDU module (scalar OIDs).
func TestEatonEpduMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                       "Eaton ePDU",
		oidSysObjectID:                    ".1.3.6.1.4.1.534.6.6.7.1",
		"1.3.6.1.4.1.534.6.6.7.1.2.1.3.0": "EMAT10-10",
		"1.3.6.1.4.1.534.6.6.7.1.2.1.4.0": "WA123456",
		"1.3.6.1.4.1.534.6.6.7.1.2.1.5.0": "2.0.5",
	}}
	d, _ := GetInventory("192.0.2.41", getter)
	if d["MODEL"] != "EMAT10-10" || d["SERIAL"] != "WA123456" || d["FIRMWARE"] != "2.0.5" {
		t.Errorf("eaton = %v", d)
	}
}

// TestInfortrendMibSupport checks a STORAGE module with composed firmware.
func TestInfortrendMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                     "Infortrend",
		oidSysObjectID:                  ".1.3.6.1.4.1.1714.1.1.1",
		"1.3.6.1.4.1.1714.1.1.1.1.4.0":  "3",  // fw major
		"1.3.6.1.4.1.1714.1.1.1.1.5.0":  "88", // fw minor
		"1.3.6.1.4.1.1714.1.1.1.1.10.0": "SN-INF-001",
		"1.3.6.1.4.1.1714.1.1.1.1.15.0": "EonStor GS",
	}}
	d, _ := GetInventory("192.0.2.50", getter)
	if d["TYPE"] != "STORAGE" || d["FIRMWARE"] != "3.88" || d["SERIAL"] != "SN-INF-001" || d["MODEL"] != "EonStor GS" {
		t.Errorf("infortrend = %v", d)
	}
}

// TestUpsApcSerialFallback checks the APC UPS serial fallback chain.
func TestUpsApcSerialFallback(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                   "APC Web/SNMP",
		oidSysObjectID:                ".1.3.6.1.4.1.318.1.3",
		"1.3.6.1.4.1.318.1.1.4.1.4.0": "AP7921",    // model
		"1.3.6.1.4.1.318.1.1.4.1.5.0": "ZA1234567", // sPDU serial (fallback)
		"1.3.6.1.4.1.318.1.1.4.1.2.0": "v3.7.3",    // firmware
	}}
	d, _ := GetInventory("192.0.2.51", getter)
	if d["MODEL"] != "AP7921" || d["SERIAL"] != "ZA1234567" || d["FIRMWARE"] != "v3.7.3" {
		t.Errorf("apc = %v", d)
	}
}

// TestHpCitizenMibSupport checks a STORAGE module with manufacturer/model from OIDs.
func TestHpCitizenMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                     "HP storage",
		oidSysObjectID:                  ".1.3.6.1.4.1.11.10.1",
		"1.3.6.1.4.1.11.2.36.1.1.2.4.0": "HP",
		"1.3.6.1.4.1.11.2.36.1.1.2.5.0": "StoreEasy",
		"1.3.6.1.4.1.11.2.36.1.1.2.6.0": "1.2.3",
		"1.3.6.1.4.1.11.2.36.1.1.2.9.0": "CZ12345",
	}}
	d, _ := GetInventory("192.0.2.60", getter)
	if d["TYPE"] != "STORAGE" || d["MANUFACTURER"] != "HP" || d["MODEL"] != "StoreEasy" || d["SERIAL"] != "CZ12345" {
		t.Errorf("hp-citizen = %v", d)
	}
}

// TestDigiMibSupport checks the Digi Sarian router module.
func TestDigiMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                      "Digi TransPort",
		oidSysObjectID:                   ".1.3.6.1.4.1.16378.10000.5",
		"1.3.6.1.4.1.16378.10000.3.15.0": "TWX-MK4-1234",
		"1.3.6.1.4.1.16378.10000.3.16.0": "8.1.2.3",
	}}
	d, _ := GetInventory("192.0.2.61", getter)
	if d["SERIAL"] != "TWX-MK4-1234" || d["FIRMWARE"] != "8.1.2.3" {
		t.Errorf("digi = %v", d)
	}
}

// TestHikvisionMibSupport checks the camera identity: NETWORKING + Hikvision,
// model from the private OID, serial from the entity index, MAC normalised, and
// SNMPHOSTNAME composed as MODEL_serial.
func TestHikvisionMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:               "DS-2CD2042WD-I",
		oidSysObjectID:            ".1.3.6.1.4.1.39165.1",
		"1.3.6.1.4.1.39165.1.1.0": "DS-2CD2042WD-I",    // model
		"1.3.6.1.4.1.39165.1.4.0": "AA-BB-CC-DD-EE-FF", // mac
		"1.3.6.1.4.1.50001.1.3.0": "SN-ENTITY-001",     // entity index -> serial
	}}
	d, err := GetInventory("192.0.2.70", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["TYPE"] != "NETWORKING" || d["MANUFACTURER"] != "Hikvision" {
		t.Errorf("type/manufacturer = %v/%v", d["TYPE"], d["MANUFACTURER"])
	}
	if d["MODEL"] != "DS-2CD2042WD-I" {
		t.Errorf("MODEL = %v", d["MODEL"])
	}
	if d["SERIAL"] != "SN-ENTITY-001" {
		t.Errorf("SERIAL = %v, want the entity index", d["SERIAL"])
	}
	if d["MAC"] != "aa:bb:cc:dd:ee:ff" {
		t.Errorf("MAC = %v, want normalised", d["MAC"])
	}
	if d["SNMPHOSTNAME"] != "DS-2CD2042WD-I_SN-ENTITY-001" {
		t.Errorf("SNMPHOSTNAME = %v", d["SNMPHOSTNAME"])
	}
}

// TestHikvisionSerialFallback verifies the serial falls back to the MAC (dashes
// stripped) when no entity index is present.
func TestHikvisionSerialFallback(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:               "Hikvision NVR",
		oidSysObjectID:            ".1.3.6.1.4.1.39165.1",
		"1.3.6.1.4.1.39165.1.4.0": "AA-BB-CC-DD-EE-FF",
	}}
	d, _ := GetInventory("192.0.2.71", getter)
	if d["SERIAL"] != "AABBCCDDEEFF" {
		t.Errorf("SERIAL = %v, want the MAC without dashes", d["SERIAL"])
	}
}

// TestSiemensSicamMibSupport checks the identity parsed out of the sysDescr.
func TestSiemensSicamMibSupport(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:    "Siemens AG, SICAM A8000, CP-8050, HW1, FW: 4.50, SN: VPV1234567",
		oidSysObjectID: ".1.3.6.1.4.1.22638.1",
	}}
	d, err := GetInventory("192.0.2.72", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["TYPE"] != "NETWORKING" || d["MANUFACTURER"] != "Siemens" {
		t.Errorf("type/manufacturer = %v/%v", d["TYPE"], d["MANUFACTURER"])
	}
	if d["MODEL"] != "SICAM A8000 CP-8050" {
		t.Errorf("MODEL = %v", d["MODEL"])
	}
	if d["FIRMWARE"] != "4.50" {
		t.Errorf("FIRMWARE = %v", d["FIRMWARE"])
	}
	if d["SERIAL"] != "VPV1234567" {
		t.Errorf("SERIAL = %v", d["SERIAL"])
	}
}

// TestXeroxPageCounters checks the run hook sums the XEROX-HOST-RESOURCES-EXT
// counters into PAGECOUNTERS and derives COPYTOTAL.
func TestXeroxPageCounters(t *testing.T) {
	const detail = "1.3.6.1.4.1.253.8.53.13.2.1"
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:            "Xerox WorkCentre",
		oidSysObjectID:         ".1.3.6.1.4.1.253.8.62.1",
		detail + ".6.1.20.1":   "1500", // PRINTTOTAL
		detail + ".6.1.20.33":  "400",  // PRINTCOLOR
		detail + ".6.1.20.34":  "1100", // PRINTBLACK
		detail + ".6.11.20.25": "200",  // COPYCOLOR
		detail + ".6.11.20.3":  "300",  // COPYBLACK
		detail + ".6.10.20.11": "10",   // scan by email
		detail + ".6.10.20.12": "5",    // scan on network
	}}
	d, err := GetInventory("192.0.2.80", getter)
	if err != nil {
		t.Fatal(err)
	}
	pc, ok := d["PAGECOUNTERS"].(map[string]any)
	if !ok {
		t.Fatalf("no PAGECOUNTERS: %v", d)
	}
	if pc["PRINTTOTAL"] != 1500 || pc["PRINTCOLOR"] != 400 || pc["PRINTBLACK"] != 1100 {
		t.Errorf("print counters = %v", pc)
	}
	if pc["SCANNED"] != 15 {
		t.Errorf("SCANNED = %v, want 15 (10+5)", pc["SCANNED"])
	}
	if pc["COPYTOTAL"] != 500 {
		t.Errorf("COPYTOTAL = %v, want 500 (300+200)", pc["COPYTOTAL"])
	}
}

// TestSiemensSicamComponents checks the DGPI product-component walk builds
// COMPONENTS and rewrites FIRMWARES.
func TestSiemensSicamComponents(t *testing.T) {
	const entry = "1.3.6.1.4.1.22638.11.1.2.1.1"
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "Siemens AG, SICAM A8000, CP-8050, HW1, FW: 4.50, SN: VPV1234567",
			oidSysObjectID: ".1.3.6.1.4.1.22638.1",
		},
		walks: map[string]map[string]string{
			entry + ".2": {"1": "0", "2": "1"},               // containedIn
			entry + ".3": {"1": "3", "2": "6"},               // class -> mainHwComponent, mainFwSwComponent
			entry + ".4": {"1": "CP-8050", "2": "Firmware"},  // name
			entry + ".5": {"1": "Master module", "2": "App"}, // description
			entry + ".7": {"1": "SN-CP-8050"},                // serial (unit 1 only)
			entry + ".8": {"2": "4.50"},                      // version (unit 2 only)
			entry + ".9": {"1": "X1"},                        // hw slot
		},
	}
	d, err := GetInventory("192.0.2.81", getter)
	if err != nil {
		t.Fatal(err)
	}
	container, _ := d["COMPONENTS"].(map[string]any)
	comps, _ := container["COMPONENT"].([]map[string]any)
	if len(comps) != 2 {
		t.Fatalf("COMPONENTS = %v", container)
	}
	if comps[0]["NAME"] != "CP-8050" || comps[0]["SERIAL"] != "SN-CP-8050" || comps[0]["TYPE"] != "mainHwComponent" {
		t.Errorf("component[0] = %v", comps[0])
	}
	if comps[1]["FIRMWARE"] != "4.50" {
		t.Errorf("component[1] firmware = %v", comps[1])
	}
	fws, _ := d["FIRMWARES"].([]map[string]any)
	if len(fws) != 1 || fws[0]["VERSION"] != "4.50" || fws[0]["MANUFACTURER"] != "Siemens" {
		t.Errorf("FIRMWARES = %v", fws)
	}
}

// TestNetgearStackSerials checks the run hook fills per-unit chassis serials
// when a stack exposes more than one chassis component.
func TestNetgearStackSerials(t *testing.T) {
	const status = "1.3.6.1.4.1.4526.10.13.2.2.1.11"
	const serial = "1.3.6.1.4.1.4526.10.13.2.2.1.19"
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "Netgear Switch",
			oidSysObjectID: ".1.3.6.1.4.1.4526.100.1",
		},
		walks: map[string]map[string]string{
			sysORID: {"1": ".1.3.6.1.4.1.4526.10.13"}, // advertise the inventory mib
			status:  {"1": "1", "2": "1"},
			serial:  {"1": "SER-UNIT-1", "2": "SER-UNIT-2"},
		},
	}
	d, err := GetInventory("192.0.2.82", getter)
	if err != nil {
		t.Fatal(err)
	}
	// Seed two chassis components (as the generic ENTITY-MIB step would).
	addComponent(d, map[string]any{"TYPE": "chassis", "NAME": "Unit 1"})
	addComponent(d, map[string]any{"TYPE": "chassis", "NAME": "Unit 2"})
	mods := matchMibModules(".1.3.6.1.4.1.4526.100.1", map[string]bool{"1.3.6.1.4.1.4526.10.13": true}, getter)
	runMibSupport(d, getter, mods)

	comps := d["COMPONENTS"].(map[string]any)["COMPONENT"].([]map[string]any)
	if comps[0]["SERIAL"] != "SER-UNIT-1" || comps[0]["STACK_NUMBER"] != "1" {
		t.Errorf("unit 1 = %v", comps[0])
	}
	if comps[1]["SERIAL"] != "SER-UNIT-2" || comps[1]["STACK_NUMBER"] != "2" {
		t.Errorf("unit 2 = %v", comps[1])
	}
}

// TestEMCMibSupport checks the FCMGMT connUnit table drives TYPE/SERIAL/MODEL,
// keyed by the lowest connUnit index.
func TestEMCMibSupport(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:              "EMC storage",
			oidSysObjectID:           ".1.3.6.1.4.1.674.11000",
			"1.3.6.1.3.94.1.6.1.8.1": "EMC-SN-001", // connUnitSn.1
			"1.3.6.1.3.94.1.6.1.7.1": "VNX5400",    // connUnitProduct.1
		},
		walks: map[string]map[string]string{
			"1.3.6.1.3.94.1.6.1.1": {"1": "unit-a", "2": "unit-b"}, // connUnitId
		},
	}
	d, err := GetInventory("192.0.2.90", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["TYPE"] != "NETWORKING" || d["SERIAL"] != "EMC-SN-001" || d["MODEL"] != "VNX5400" {
		t.Errorf("emc = %v", d)
	}
}

// TestForce10Components checks the stack-unit + port + root component build.
func TestForce10Components(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "Force10 S4810",
			oidSysObjectID: ".1.3.6.1.4.1.6027.1.3.10",
		},
		walks: map[string]map[string]string{
			"1.3.6.1.4.1.6027.3.10.1.2.2.1": entWalk(map[string]map[string]string{
				"1": {"2": "1", "7": "S4810", "12": "FORCE-SN-1"},
			}),
			"1.3.6.1.4.1.6027.3.10.1.2.5.1.5": {"1.0": "101"}, // suffix .stack(1).port(0) -> ifIndex 101
		},
	}
	d, err := GetInventory("192.0.2.91", getter)
	if err != nil {
		t.Fatal(err)
	}
	comps := d["COMPONENTS"].(map[string]any)["COMPONENT"].([]map[string]any)
	// chassis + port + root stack.
	if len(comps) != 3 {
		t.Fatalf("components = %v", comps)
	}
	if comps[0]["TYPE"] != "chassis" || comps[0]["MODEL"] != "S4810" || comps[0]["NAME"] != "0" {
		t.Errorf("chassis = %v", comps[0])
	}
	if comps[1]["TYPE"] != "port" || comps[1]["INDEX"] != "101" || comps[1]["CONTAINEDININDEX"] != "1" {
		t.Errorf("port = %v", comps[1])
	}
	if comps[2]["TYPE"] != "stack" || comps[2]["INDEX"] != "-1" {
		t.Errorf("root = %v", comps[2])
	}
}

// TestPanasasSerial checks the cluster member serial is selected by the device IP.
func TestPanasasSerial(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:                         "Panasas",
			oidSysObjectID:                      ".1.3.6.1.4.1.10159.1.3.0",
			"1.3.6.1.4.1.10159.1.3.2.1.1.0":     "panfs-cluster", // cluster name
			"1.3.6.1.4.1.10159.1.3.2.1.3.1.3.7": "BLADE-SN-7",    // blade SN at index 7
		},
		walks: map[string]map[string]string{
			"1.3.6.1.4.1.10159.1.3.2.1.3.1.2": {"5": "10.0.0.1", "7": "192.0.2.92"}, // repset IPs
		},
	}
	d, err := GetInventory("192.0.2.92", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["SERIAL"] != "BLADE-SN-7" {
		t.Errorf("SERIAL = %v, want BLADE-SN-7 (matched by IP)", d["SERIAL"])
	}
	if d["NAME"] != "panfs-cluster" {
		t.Errorf("NAME = %v", d["NAME"])
	}
}

// TestSiemensModule checks the MLFB model map and the MAC-derived serial fallback.
func TestSiemensModule(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                              "Siemens AS-i",
		oidSysObjectID:                           ".1.3.6.1.4.1.4196.1.1",
		"1.3.6.1.4.1.4196.1.1.8.3.100.1.8.26.0":  "6GK1 411-2AB10",    // MLFB -> known model
		"1.3.6.1.4.1.4196.1.1.8.3.100.1.10.10.0": "AA:BB:CC:DD:EE:FF", // mac base
	}}
	d, err := GetInventory("192.0.2.93", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["TYPE"] != "NETWORKING" || d["MANUFACTURER"] != "Siemens" {
		t.Errorf("type/manufacturer = %v/%v", d["TYPE"], d["MANUFACTURER"])
	}
	if d["MODEL"] != "IE/AS-i LINK PN IO" {
		t.Errorf("MODEL = %v", d["MODEL"])
	}
	// No serial OID -> MAC without colons.
	if d["SERIAL"] != "aabbccddeeff" {
		t.Errorf("SERIAL = %v, want the MAC fallback", d["SERIAL"])
	}
}

// TestFreeBSDStormshield checks the Stormshield-gated identity accessors.
func TestFreeBSDStormshield(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{
		oidSysDescr:                 "FreeBSD firewall",
		oidSysObjectID:              ".1.3.6.1.4.1.8072.3.2.8",
		"1.3.6.1.4.1.11256.1.0.1.0": "SN-3100",    // model -> is_stormshield
		"1.3.6.1.4.1.11256.1.0.2.0": "4.3.5",      // firmware
		"1.3.6.1.4.1.11256.1.0.3.0": "STORM-SN-1", // serial
		"1.3.6.1.4.1.11256.1.0.4.0": "fw-paris",   // name
	}}
	d, err := GetInventory("192.0.2.94", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["TYPE"] != "NETWORKING" || d["MANUFACTURER"] != "StormShield" {
		t.Errorf("type/manufacturer = %v/%v", d["TYPE"], d["MANUFACTURER"])
	}
	if d["MODEL"] != "SN-3100" || d["FIRMWARE"] != "4.3.5" || d["SERIAL"] != "STORM-SN-1" {
		t.Errorf("freebsd = %v", d)
	}
	if d["NAME"] != "fw-paris" {
		t.Errorf("NAME = %v", d["NAME"])
	}
}

func containsModule(mods []MibModule, name string) bool {
	for _, m := range mods {
		if m.Name == name {
			return true
		}
	}
	return false
}
