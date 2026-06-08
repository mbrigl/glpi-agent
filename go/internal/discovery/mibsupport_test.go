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

func containsModule(mods []MibModule, name string) bool {
	for _, m := range mods {
		if m.Name == name {
			return true
		}
	}
	return false
}
