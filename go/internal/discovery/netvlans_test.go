// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// vlansOf returns PORT.VLANS.VLAN for the given ifnumber.
func vlansOf(d Device, ifnum string) []map[string]any {
	port := portsByNumber(d)[ifnum]
	if port == nil {
		return nil
	}
	v, _ := port["VLANS"].(map[string]any)
	if v == nil {
		return nil
	}
	list, _ := v["VLAN"].([]map[string]any)
	return list
}

// TestDot1qVlans checks the 802.1Q egress/untagged bitmap path: tagged vs
// untagged membership per port.
func TestDot1qVlans(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1", "2": "Gi0/2", "3": "Gi0/3"}, map[string]map[string]string{
		oidDot1qVlanStaticName:          {"10": "Data", "20": "Voice"},
		oidDot1qVlanStaticRowStatus:     {"10": "1", "20": "1"},
		oidDot1qVlanStaticEgressPorts:   {"10": "c0:00", "20": "20:00"}, // 10:ports1,2  20:port3
		oidDot1qVlanStaticUntaggedPorts: {"10": "80:00", "20": "20:00"}, // 10:port1     20:port3
	})
	d, err := GetInventory("192.0.2.20", getter)
	if err != nil {
		t.Fatal(err)
	}

	// Port 1: untagged member of VLAN 10.
	p1 := vlansOf(d, "1")
	if len(p1) != 1 || p1[0]["NUMBER"] != "10" || p1[0]["NAME"] != "Data" || p1[0]["TAGGED"] != 0 {
		t.Errorf("port1 vlans = %v", p1)
	}
	// Port 2: tagged member of VLAN 10.
	p2 := vlansOf(d, "2")
	if len(p2) != 1 || p2[0]["NUMBER"] != "10" || p2[0]["TAGGED"] != 1 {
		t.Errorf("port2 vlans = %v", p2)
	}
	// Port 3: untagged member of VLAN 20.
	p3 := vlansOf(d, "3")
	if len(p3) != 1 || p3[0]["NUMBER"] != "20" || p3[0]["NAME"] != "Voice" || p3[0]["TAGGED"] != 0 {
		t.Errorf("port3 vlans = %v", p3)
	}
}

// TestCiscoVtpVlans checks the Cisco VTP per-port vlan-id path.
func TestCiscoVtpVlans(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1", "2": "Gi0/2"}, map[string]map[string]string{
		oidVtpVlanName:  {"100": "Servers", "200": "Guests"},
		oidVmPortStatus: {"1.1": "100", "1.2": "200"}, // port 1 -> vlan100, port 2 -> vlan200
	})
	d, err := GetInventory("192.0.2.21", getter)
	if err != nil {
		t.Fatal(err)
	}
	p1 := vlansOf(d, "1")
	if len(p1) != 1 || p1[0]["NUMBER"] != "100" || p1[0]["NAME"] != "Servers" {
		t.Errorf("port1 vtp vlans = %v", p1)
	}
	p2 := vlansOf(d, "2")
	if len(p2) != 1 || p2[0]["NUMBER"] != "200" || p2[0]["NAME"] != "Guests" {
		t.Errorf("port2 vtp vlans = %v", p2)
	}
}

// TestOctetBits checks the bitmap expansion of a colon-hex octet string (the
// form multi-byte SNMP port bitmaps arrive in).
func TestOctetBits(t *testing.T) {
	if got := octetBits("c0:00"); got != "1100000000000000" {
		t.Errorf("octetBits(c0:00) = %q", got)
	}
	if got := octetBits("ff:01"); got != "1111111100000001" {
		t.Errorf("octetBits(ff:01) = %q", got)
	}
}
