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

func containsModule(mods []MibModule, name string) bool {
	for _, m := range mods {
		if m.Name == name {
			return true
		}
	}
	return false
}
