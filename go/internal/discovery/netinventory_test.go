// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// TestGetInventoryPorts checks that the IF-MIB walk is assembled into the PORTS
// table (one entry per interface index, with IFNAME falling back to IFDESCR).
func TestGetInventoryPorts(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr: "Switch X",
			oidSysName:  "sw-x",
		},
		walks: map[string]map[string]string{
			oidIfDescr:       {"1": "GigabitEthernet0/1", "2": "GigabitEthernet0/2"},
			oidIfType:        {"1": "6", "2": "6"},
			oidIfSpeed:       {"1": "1000000000", "2": "1000000000"},
			oidIfPhysAddress: {"1": "00:11:22:33:44:55"},
			oidIfName:        {"1": "Gi0/1"},
		},
	}

	device, err := GetInventory("192.0.2.5", getter)
	if err != nil {
		t.Fatal(err)
	}
	if device == nil {
		t.Fatal("device is nil")
	}

	ports, ok := device["PORTS"].([]map[string]any)
	if !ok {
		t.Fatalf("PORTS missing or wrong type: %T", device["PORTS"])
	}
	if len(ports) != 2 {
		t.Fatalf("got %d ports, want 2", len(ports))
	}

	// Sorted by ifIndex: port 1 first.
	p1 := ports[0]
	if p1["IFNUMBER"] != "1" || p1["IFNAME"] != "Gi0/1" || p1["MAC"] != "00:11:22:33:44:55" {
		t.Errorf("port 1 = %v", p1)
	}
	if p1["IFDESCR"] != "GigabitEthernet0/1" || p1["IFSPEED"] != "1000000000" {
		t.Errorf("port 1 details = %v", p1)
	}

	// Port 2 has no ifName -> falls back to ifDescr.
	p2 := ports[1]
	if p2["IFNUMBER"] != "2" || p2["IFNAME"] != "GigabitEthernet0/2" {
		t.Errorf("port 2 IFNAME fallback wrong: %v", p2)
	}
}

// TestGetInventoryIdentity checks the sysObjectID classification and the
// device-level SERIAL/MAC enrichment.
func TestGetInventoryIdentity(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:            "Cisco IOS",
			oidSysObjectID:         ".1.3.6.1.4.1.9.1.282", // Cisco Catalyst 6506
			oidDot1dBaseBridgeAddr: "00:aa:bb:cc:dd:ee",
		},
		walks: map[string]map[string]string{
			oidEntPhysicalSerialNum: {"1": "FOX1234ABCD"},
		},
	}
	device, err := GetInventory("192.0.2.7", getter)
	if err != nil {
		t.Fatal(err)
	}
	if device["TYPE"] != "NETWORKING" || device["MANUFACTURER"] != "Cisco" || device["MODEL"] != "Catalyst 6506" {
		t.Errorf("classification wrong: TYPE=%v MANUFACTURER=%v MODEL=%v",
			device["TYPE"], device["MANUFACTURER"], device["MODEL"])
	}
	if device["SERIAL"] != "FOX1234ABCD" {
		t.Errorf("SERIAL = %v, want FOX1234ABCD", device["SERIAL"])
	}
	if device["MAC"] != "00:aa:bb:cc:dd:ee" {
		t.Errorf("MAC = %v, want the bridge address", device["MAC"])
	}
}

// TestGetInventoryNoSNMP returns nil when the device does not answer.
func TestGetInventoryNoSNMP(t *testing.T) {
	getter := &fakeGetter{values: map[string]string{}}
	device, err := GetInventory("192.0.2.6", getter)
	if err != nil {
		t.Fatal(err)
	}
	if device != nil {
		t.Errorf("expected nil device, got %v", device)
	}
}
