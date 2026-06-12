// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// connectionMACs returns PORT.CONNECTIONS.CONNECTION.MAC for an ifnumber.
func connectionMACs(d Device, ifnum string) []string {
	conn := connectionOf(d, ifnum)
	if conn == nil {
		return nil
	}
	macs, _ := conn["MAC"].([]string)
	return macs
}

// TestPerVlanFdbSwitching checks that when the dot1q FDB is empty, the FDB is
// re-read per VLAN by switching the SNMP context (community@vlan), and the
// learned MACs are attached to the ports.
func TestPerVlanFdbSwitching(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1", "2": "Gi0/2"}, map[string]map[string]string{
		// VTP VLANs so both ports carry VLAN 100 (vlansToScan finds it).
		oidVtpVlanName:  {"100": "Servers"},
		oidVmPortStatus: {"1.1": "100", "1.2": "100"},
		// No dot1d/dot1q FDB in the default context.
	})
	// The VLAN-100 context exposes a learned MAC on bridge port 1 (ifIndex 1).
	getter.vlanWalks = map[string]map[string]map[string]string{
		"100": {
			oidDot1dTpFdbAddress:    {"6.0.0.0.0.1": "aa:bb:cc:00:00:01"},
			oidDot1dTpFdbPort:       {"6.0.0.0.0.1": "1"},
			oidDot1dTpFdbStatus:     {"6.0.0.0.0.1": "3"}, // learned
			oidDot1dBasePortIfIndex: {"1": "1"},
		},
	}

	d, err := GetInventory("192.0.2.30", getter)
	if err != nil {
		t.Fatal(err)
	}
	macs := connectionMACs(d, "1")
	found := false
	for _, m := range macs {
		if m == "aa:bb:cc:00:00:01" {
			found = true
		}
	}
	if !found {
		t.Errorf("port 1 CONNECTION.MAC = %v, want the VLAN-100 learned MAC", macs)
	}
}

// TestVlansToScan checks the VLAN selection excludes the default VLAN and
// CDP-identified ports.
func TestVlansToScan(t *testing.T) {
	byNum := map[string]map[string]any{
		"1": {"VLANS": map[string]any{"VLAN": []map[string]any{{"NUMBER": "1"}, {"NUMBER": "10"}}}},
		"2": {"VLANS": map[string]any{"VLAN": []map[string]any{{"NUMBER": "20"}}},
			"CONNECTIONS": map[string]any{"CDP": 1}}, // CDP -> skipped
		"3": {"VLANS": map[string]any{"VLAN": []map[string]any{{"NUMBER": "10"}, {"NUMBER": "30"}}}},
	}
	vlans := vlansToScan(byNum)
	// Expect 10, 30 (1 is default, 20 is on a CDP port, 10 deduped).
	got := map[string]bool{}
	for _, v := range vlans {
		got[v] = true
	}
	if got["1"] || got["20"] || !got["10"] || !got["30"] || len(vlans) != 2 {
		t.Errorf("vlansToScan = %v, want [10 30]", vlans)
	}
}
