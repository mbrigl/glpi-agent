// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func loadIfconfig(t *testing.T, name string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "ifconfig", name))
	if err != nil {
		t.Fatalf("read ifconfig %s: %v", name, err)
	}
	return string(data)
}

// TestBuildMacNetworks pins the ifconfig+networksetup parser against the real
// captures, using the expected values from t/tasks/inventory/macos/networks.t.
func TestBuildMacNetworks(t *testing.T) {
	netsetup := parseMacNetworkSetup(loadIfconfig(t, "macosx-01-networksetup"))
	ifaces := buildMacNetworks(loadIfconfig(t, "macosx-01"), netsetup)

	byDesc := map[string]map[string]any{}
	for _, i := range ifaces {
		byDesc[i["DESCRIPTION"].(string)] = i
	}

	lo := byDesc["lo0"]
	wantLo := map[string]any{
		"IPADDRESS": "127.0.0.1", "IPADDRESS6": "fe80::1", "IPMASK": "255.0.0.0",
		"IPSUBNET": "127.0.0.0", "TYPE": "loopback", "MTU": 16384, "STATUS": "Down", "VIRTUALDEV": 1,
	}
	for k, v := range wantLo {
		if lo[k] != v {
			t.Errorf("lo0[%s] = %v, want %v", k, lo[k], v)
		}
	}

	eth := byDesc["Ethernet"]
	wantEth := map[string]any{
		"IPADDRESS": "172.77.220.189", "IPADDRESS6": "fe80::10f6:f9c8:4818:4587",
		"IPMASK": "255.255.255.0", "IPSUBNET": "172.77.220.0", "MACADDR": "0c:4d:e9:c9:6c:3c",
		"TYPE": "ethernet", "MTU": 1500, "SPEED": 100, "STATUS": "Up",
	}
	for k, v := range wantEth {
		if eth[k] != v {
			t.Errorf("Ethernet[%s] = %v, want %v", k, eth[k], v)
		}
	}
}

// TestHexToDottedMask covers the netmask conversion.
func TestHexToDottedMask(t *testing.T) {
	cases := map[string]string{
		"ffffff00": "255.255.255.0",
		"ff000000": "255.0.0.0",
		"fffffe00": "255.255.254.0",
	}
	for in, want := range cases {
		if got := hexToDottedMask(in); got != want {
			t.Errorf("hexToDottedMask(%q) = %q, want %q", in, got, want)
		}
	}
}
