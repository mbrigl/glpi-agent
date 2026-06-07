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
	bySysobj := matchMibModules(".1.3.6.1.4.1.41112.1.6", map[string]bool{})
	if !containsModule(bySysobj, "ubnt") {
		t.Error("ubnt should match by sysObjectID")
	}
	byOrid := matchMibModules("", map[string]bool{"1.3.6.1.4.1.41112": true})
	if !containsModule(byOrid, "ubnt") {
		t.Error("ubnt should match by sysORID oid rule")
	}
	if len(matchMibModules(".1.2.3.4", map[string]bool{})) != 0 {
		t.Error("an unrelated sysObjectID must match nothing")
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
