// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// netGetter builds a NETWORKING device fixture (so setNetworkingProperties runs)
// with the given IF-MIB ports and extra walks.
func netGetter(ports map[string]string, walks map[string]map[string]string) *fakeGetter {
	w := map[string]map[string]string{oidIfDescr: ports}
	for k, v := range walks {
		w[k] = v
	}
	return &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "Switch",
			oidSysObjectID: ".1.3.6.1.4.1.9.1.1", // Cisco -> NETWORKING
		},
		walks: w,
	}
}

func portsByNumber(d Device) map[string]map[string]any {
	out := map[string]map[string]any{}
	ports, _ := d["PORTS"].([]map[string]any)
	for _, p := range ports {
		out[p["IFNUMBER"].(string)] = p
	}
	return out
}

// TestTrunkPortsCisco checks the Cisco trunk-status table sets PORT.TRUNK.
func TestTrunkPortsCisco(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1", "2": "Gi0/2"}, map[string]map[string]string{
		oidVlanTrunkPortDynStatus: {"1": "1", "2": "2"}, // 1 trunk, 2 access
	})
	d, err := GetInventory("192.0.2.130", getter)
	if err != nil {
		t.Fatal(err)
	}
	p := portsByNumber(d)
	if p["1"]["TRUNK"] != 1 {
		t.Errorf("port 1 TRUNK = %v, want 1", p["1"]["TRUNK"])
	}
	if p["2"]["TRUNK"] != 0 {
		t.Errorf("port 2 TRUNK = %v, want 0", p["2"]["TRUNK"])
	}
}

// TestAggregatePortsLACP checks LACP members are grouped under the aggregator.
func TestAggregatePortsLACP(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1", "2": "Gi0/2", "3": "Po1"}, map[string]map[string]string{
		// interfaces 1 and 2 are attached to aggregator 3.
		oidAggPortAttachedAggID: {"1": "3", "2": "3", "3": "0"},
	})
	d, _ := GetInventory("192.0.2.131", getter)
	p := portsByNumber(d)
	agg, _ := p["3"]["AGGREGATE"].(map[string]any)
	members, _ := agg["PORT"].([]string)
	if len(members) != 2 {
		t.Fatalf("aggregate members = %v", p["3"]["AGGREGATE"])
	}
	got := map[string]bool{members[0]: true, members[1]: true}
	if !got["1"] || !got["2"] {
		t.Errorf("aggregate members = %v, want 1 and 2", members)
	}
}

// TestAggregatePortsPAGP checks the PAgP short-number + 5000 aggregate id.
func TestAggregatePortsPAGP(t *testing.T) {
	getter := netGetter(map[string]string{"10": "Gi0/10", "5001": "Po1"}, map[string]map[string]string{
		oidPagpPorts: {"10": "1"}, // port 10 -> group 1 -> aggregate 5001
	})
	d, _ := GetInventory("192.0.2.132", getter)
	p := portsByNumber(d)
	agg, _ := p["5001"]["AGGREGATE"].(map[string]any)
	members, _ := agg["PORT"].([]string)
	if len(members) != 1 || members[0] != "10" {
		t.Errorf("PAgP aggregate = %v", p["5001"]["AGGREGATE"])
	}
}

// TestKnownMacAddresses checks the dot1d FDB attaches learned MACs as a port
// connection, mapped through the bridge-port table, and filters the status.
func TestKnownMacAddresses(t *testing.T) {
	getter := netGetter(map[string]string{"11": "Gi0/1"}, map[string]map[string]string{
		oidDot1dBasePortIfIndex: {"1": "11"}, // bridge port 1 -> interface 11
		oidDot1dTpFdbAddress:    {"0.1.2.3.4.5.6": "aa:bb:cc:dd:ee:ff", "0.1.2.3.4.5.7": "11:22:33:44:55:66"},
		oidDot1dTpFdbPort:       {"0.1.2.3.4.5.6": "1", "0.1.2.3.4.5.7": "1"},
		oidDot1dTpFdbStatus:     {"0.1.2.3.4.5.6": "3", "0.1.2.3.4.5.7": "1"}, // learned, invalid(1)
	})
	d, _ := GetInventory("192.0.2.133", getter)
	p := portsByNumber(d)
	macs := portConnectionMACs(p["11"])
	if len(macs) != 1 || macs[0] != "aa:bb:cc:dd:ee:ff" {
		t.Errorf("known MACs = %v, want only the learned one", macs)
	}
}

// TestKnownMacFromSuffix checks the decimal-byte MAC recovery when the address
// table is absent.
func TestKnownMacFromSuffix(t *testing.T) {
	if got := macFromSuffix("5.0.27.68.17.34.51"); got != "00:1b:44:11:22:33" {
		t.Errorf("macFromSuffix = %q", got)
	}
	if macFromSuffix("1.2.3") != "" {
		t.Error("short suffix should yield no MAC")
	}
}

// TestNetworkingPropsSkippedForNonNetworking verifies the enrichment does not run
// for a non-NETWORKING device.
func TestNetworkingPropsSkippedForNonNetworking(t *testing.T) {
	getter := &fakeGetter{
		values: map[string]string{
			oidSysDescr:    "Printer",
			oidSysObjectID: ".1.3.6.1.4.1.1602.1", // Canon -> PRINTER
		},
		walks: map[string]map[string]string{
			oidIfDescr:                {"1": "eth0"},
			oidVlanTrunkPortDynStatus: {"1": "1"},
		},
	}
	d, _ := GetInventory("192.0.2.134", getter)
	p := portsByNumber(d)
	if _, ok := p["1"]["TRUNK"]; ok {
		t.Error("TRUNK should not be set on a non-NETWORKING device")
	}
}
