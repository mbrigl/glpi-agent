// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"os"
	"path/filepath"
	"testing"
)

// findMib returns the registered MibSupport module with the given name.
func findMib(t *testing.T, name string) *MibModule {
	t.Helper()
	for i := range mibRegistry {
		if mibRegistry[i].Name == name {
			return &mibRegistry[i]
		}
	}
	t.Fatalf("MibSupport module %q not registered", name)
	return nil
}

// TestForce10SRealWalk replays the upstream force10s.walk capture through the
// Force10S getComponents accessor and asserts the same components the upstream
// t/tasks/netinventory/mibsupport/force10s.t expects (cmp_bag, unordered): the
// 8 stack chassis (with their serials), 24 ports and the root stack node.
func TestForce10SRealWalk(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("testdata", "walks", "force10s.walk"))
	if err != nil {
		t.Fatal(err)
	}
	g := &walkGetter{values: parseWalk(string(data))}

	mod := findMib(t, "Force10 S-series")
	if mod.Components == nil {
		t.Fatal("Force10S has no Components accessor")
	}
	comps := mod.Components(g, Device{})

	var chassis, ports int
	var root map[string]any
	serials := map[string]bool{}
	var unit1 map[string]any
	for _, c := range comps {
		switch c["TYPE"] {
		case "chassis":
			chassis++
			if s, _ := c["SERIAL"].(string); s != "" {
				serials[s] = true
			}
			if c["INDEX"] == "1" {
				unit1 = c
			}
		case "port":
			ports++
		case "stack":
			root = c
		}
	}

	if len(comps) != 33 {
		t.Errorf("total components = %d, want 33 (8 chassis + 24 ports + root)", len(comps))
	}
	if chassis != 8 {
		t.Errorf("chassis = %d, want 8", chassis)
	}
	if ports != 24 {
		t.Errorf("ports = %d, want 24", ports)
	}
	if root == nil || root["INDEX"] != "-1" || root["NAME"] != "Force10 S-series Stack" || root["CONTAINEDININDEX"] != "0" {
		t.Errorf("root stack = %v", root)
	}

	wantSerials := []string{
		"DL250170022", "DL251050115", "DL253170068", "DL253170039",
		"DL253170089", "DL253170071", "DL251050010", "DL251280022",
	}
	for _, s := range wantSerials {
		if !serials[s] {
			t.Errorf("missing chassis serial %q (have %v)", s, serials)
		}
	}

	// Full field check of the first stack unit (trailing spaces trimmed).
	if unit1 == nil {
		t.Fatal("chassis INDEX 1 not found")
	}
	want := map[string]any{
		"CONTAINEDININDEX": "-1",
		"INDEX":            "1",
		"NAME":             "0", // chassis number in interface names starts at 0
		"TYPE":             "chassis",
		"MODEL":            "S50-01-GE-48T-AC",
		"DESCRIPTION":      "48-port E/FE/GE (SB)",
		"FIRMWARE":         "8.4.2.7",
		"SERIAL":           "DL250170022",
		"REVISION":         "D",
	}
	for k, v := range want {
		if unit1[k] != v {
			t.Errorf("unit1[%s] = %v, want %v", k, unit1[k], v)
		}
	}
}

// TestUbntRealWalk replays the upstream sample7.walk (a UniFi AP) through the
// full GetInventory path and asserts the Ubnt run hook annotates the WiFi radio
// ports the same way t/tasks/netinventory/mibsupport/ubnt.t expects: IFTYPE
// fixed to 71, IFALIAS = interface name, IFNAME = SSID + band/VLAN.
func TestUbntRealWalk(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("testdata", "walks", "sample7.walk"))
	if err != nil {
		t.Fatal(err)
	}
	g := &walkGetter{values: parseWalk(string(data))}

	device, err := GetInventory("10.0.0.1", g)
	if err != nil {
		t.Fatal(err)
	}
	if device == nil {
		t.Fatal("no device built from the UniFi AP walk")
	}

	ports, _ := device["PORTS"].([]map[string]any)
	byNum := map[string]map[string]any{}
	for _, p := range ports {
		byNum[p["IFNUMBER"].(string)] = p
	}

	want := map[string]struct{ ifdescr, ifname, ifalias string }{
		"6":  {"wifi0ap0", "TestNet - Visitantes_2.4Ghz (2.4GHz)", "wifi0ap0"},
		"10": {"wifi1ap4", "TestNet - Visitantes_5Ghz (5GHz)", "wifi1ap4"},
		"25": {"wifi1ap5.620", "TestNet_Corp (5GHz, VLAN 620)", "wifi1ap5.620"},
	}
	for num, w := range want {
		p := byNum[num]
		if p == nil {
			t.Errorf("port %s missing", num)
			continue
		}
		if p["IFDESCR"] != w.ifdescr {
			t.Errorf("port %s IFDESCR = %v, want %s", num, p["IFDESCR"], w.ifdescr)
		}
		if p["IFTYPE"] != "71" {
			t.Errorf("port %s IFTYPE = %v, want 71 (WiFi)", num, p["IFTYPE"])
		}
		if p["IFNAME"] != w.ifname {
			t.Errorf("port %s IFNAME = %v, want %q", num, p["IFNAME"], w.ifname)
		}
		if p["IFALIAS"] != w.ifalias {
			t.Errorf("port %s IFALIAS = %v, want %s", num, p["IFALIAS"], w.ifalias)
		}
	}
}

// TestParseWalkValues unit-tests the value rendering against the snmpwalk forms.
func TestParseWalkValues(t *testing.T) {
	walk := parseWalk(`.1.3.6.1.2.1.1.1.0 = STRING: "Force10 Networks"
.1.3.6.1.2.1.1.3.0 = Timeticks: (123456) 0:20:34.56
.1.3.6.1.2.1.2.2.1.6.1 = Hex-STRING: 00 1B 44 11 22 33
.1.3.6.1.4.1.6027.3.10.1.2.2.1.2.1 = INTEGER: 1
iso.0.8802.1.1.2.1.3.1.0 = INTEGER: up(1)`)

	checks := map[string]string{
		"1.3.6.1.2.1.1.1.0":                 "Force10 Networks",  // STRING unquoted
		"1.3.6.1.2.1.1.3.0":                 "123456",            // Timeticks -> ticks
		"1.3.6.1.2.1.2.2.1.6.1":             "00:1b:44:11:22:33", // Hex-STRING MAC -> colon hex
		"1.3.6.1.4.1.6027.3.10.1.2.2.1.2.1": "1",                 // INTEGER
		"1.0.8802.1.1.2.1.3.1.0":            "1",                 // iso alias + enum stripped
	}
	for oid, want := range checks {
		if got := walk[oid]; got != want {
			t.Errorf("%s = %q, want %q", oid, got, want)
		}
	}
}
