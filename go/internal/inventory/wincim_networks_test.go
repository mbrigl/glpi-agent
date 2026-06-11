// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinNetworks checks the adapter/config join, the per-IP emission
// (IPv4 with computed subnet, IPv6 with zone stripped), the speed/status/
// virtualdev mapping and the no-address fallback.
func TestBuildWinNetworks(t *testing.T) {
	nets := buildWinNetworks(
		loadCIMArray(t, "win32_networkadapter.json"),
		loadCIMArray(t, "win32_networkadapterconfiguration.json"),
	)
	// adapter 1 has two addresses (v4 + v6) -> 2 entries; the virtual adapter 7
	// has a MAC but no addresses -> 1 base entry. Total 3.
	if len(nets) != 3 {
		t.Fatalf("got %d network entries, want 3", len(nets))
	}

	var v4, v6, virt map[string]any
	for _, n := range nets {
		switch {
		case n["IPADDRESS"] == "192.168.1.50":
			v4 = n
		case n["IPADDRESS6"] != nil:
			v6 = n
		case n["MACADDR"] == "00:50:56:C0:00:01":
			virt = n
		}
	}

	if v4 == nil || v6 == nil || virt == nil {
		t.Fatalf("missing entries: v4=%v v6=%v virt=%v", v4 != nil, v6 != nil, virt != nil)
	}

	// IPv4 entry of the physical Intel NIC.
	wantV4 := map[string]any{
		"DESCRIPTION": "Intel(R) Ethernet Connection",
		"MACADDR":     "AA:BB:CC:DD:EE:FF",
		"STATUS":      "Up",
		"MTU":         1500,
		"IPDHCP":      "192.168.1.1",
		"IPGATEWAY":   "192.168.1.254",
		"GUID":        "{NIC-GUID-1}",
		"SPEED":       1000, // 1e9 / 1e6
		"VIRTUALDEV":  0,
		"IPADDRESS":   "192.168.1.50",
		"IPMASK":      "255.255.255.0",
		"IPSUBNET":    "192.168.1.0",
	}
	for k, v := range wantV4 {
		if v4[k] != v {
			t.Errorf("v4[%s] = %v, want %v", k, v4[k], v)
		}
	}

	// IPv6 entry: zone id stripped, same scalar fields.
	if v6["IPADDRESS6"] != "fe80::1c2:3ff:fe45:6789" {
		t.Errorf("IPADDRESS6 = %v, want zone stripped", v6["IPADDRESS6"])
	}
	if v6["MACADDR"] != "AA:BB:CC:DD:EE:FF" {
		t.Errorf("v6 MACADDR = %v", v6["MACADDR"])
	}

	// Virtual adapter (PNPDeviceID under ROOT): VIRTUALDEV=1, STATUS Down, no IP.
	if virt["VIRTUALDEV"] != 1 || virt["STATUS"] != "Down" {
		t.Errorf("virtual = %v", virt)
	}
	if _, ok := virt["IPADDRESS"]; ok {
		t.Errorf("virtual adapter should have no IPADDRESS")
	}
}
