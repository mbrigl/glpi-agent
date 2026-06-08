// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// entRow builds the entPhysicalEntry walk leaves for one row: column suffix +
// "." + index -> value. Helper for the component tests.
func entWalk(rows map[string]map[string]string) map[string]string {
	walk := map[string]string{}
	for index, cols := range rows {
		for suffix, val := range cols {
			walk[suffix+"."+index] = val
		}
	}
	return walk
}

// TestBuildPhysicalComponents checks the ENTITY-MIB walk: INDEX table, TYPE enum
// mapping and the string columns.
func TestBuildPhysicalComponents(t *testing.T) {
	getter := &fakeGetter{walks: map[string]map[string]string{
		oidEntPhysicalEntry: entWalk(map[string]map[string]string{
			// index: {col: value}
			"1": {"1": "1", "5": "3", "7": "Chassis", "11": "CHS-001", "13": "X670"},
			"2": {"1": "2", "5": "9", "7": "Module 1", "4": "1"},
		}),
	}}
	comps := BuildPhysicalComponents(getter)
	if len(comps) != 2 {
		t.Fatalf("got %d components, want 2: %v", len(comps), comps)
	}
	if comps[0]["TYPE"] != "chassis" || comps[0]["NAME"] != "Chassis" || comps[0]["SERIAL"] != "CHS-001" || comps[0]["MODEL"] != "X670" {
		t.Errorf("component[0] = %v", comps[0])
	}
	if comps[1]["TYPE"] != "module" || comps[1]["CONTAINEDININDEX"] != "1" {
		t.Errorf("component[1] = %v", comps[1])
	}
}

// TestComponentsNoEntityTable verifies a device without the table yields none.
func TestComponentsNoEntityTable(t *testing.T) {
	if c := BuildPhysicalComponents(&fakeGetter{}); c != nil {
		t.Errorf("expected nil, got %v", c)
	}
}

// TestDellChassisSerialFix checks the Dell private chassis-serial override.
func TestDellChassisSerialFix(t *testing.T) {
	getter := &fakeGetter{walks: map[string]map[string]string{
		oidEntPhysicalEntry: entWalk(map[string]map[string]string{
			"1": {"1": "1", "5": "3", "7": "Unit 1"},
			"2": {"1": "2", "5": "3", "7": "Unit 2"},
		}),
		oidDellProductSerial: {"1": "DELLSN-1", "2": "DELLSN-2"},
	}}
	comps := BuildPhysicalComponents(getter)
	if comps[0]["SERIAL"] != "DELLSN-1" || comps[1]["SERIAL"] != "DELLSN-2" {
		t.Errorf("dell serials = %v / %v", comps[0]["SERIAL"], comps[1]["SERIAL"])
	}
}

// TestNetgearStackEndToEnd checks the Netgear run hook now fills serials from the
// generic ENTITY-MIB chassis components built by GetInventory.
func TestNetgearStackEndToEnd(t *testing.T) {
	const status = "1.3.6.1.4.1.4526.10.13.2.2.1.11"
	const serial = "1.3.6.1.4.1.4526.10.13.2.2.1.19"
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "Netgear Stack",
			oidSysObjectID: ".1.3.6.1.4.1.4526.100.4.1",
		},
		walks: map[string]map[string]string{
			sysORID: {"1": ".1.3.6.1.4.1.4526.10.13"},
			oidEntPhysicalEntry: entWalk(map[string]map[string]string{
				"1": {"1": "1", "5": "3", "7": "Unit 1"},
				"2": {"1": "2", "5": "3", "7": "Unit 2"},
			}),
			status: {"1": "1", "2": "1"},
			serial: {"1": "NG-UNIT-1", "2": "NG-UNIT-2"},
		},
	}
	d, err := GetInventory("192.0.2.83", getter)
	if err != nil {
		t.Fatal(err)
	}
	comps := d["COMPONENTS"].(map[string]any)["COMPONENT"].([]map[string]any)
	if len(comps) != 2 {
		t.Fatalf("components = %v", comps)
	}
	if comps[0]["SERIAL"] != "NG-UNIT-1" || comps[0]["STACK_NUMBER"] != "1" {
		t.Errorf("unit 1 = %v", comps[0])
	}
	if comps[1]["SERIAL"] != "NG-UNIT-2" || comps[1]["STACK_NUMBER"] != "2" {
		t.Errorf("unit 2 = %v", comps[1])
	}
}
