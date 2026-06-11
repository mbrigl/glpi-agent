// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinControllers checks PCI VEN/DEV parsing, the non-PCI skip and the
// vendor+product dedupe.
func TestBuildWinControllers(t *testing.T) {
	controllers := buildWinControllers(loadCIMArray(t, "win32_controllers.json"))
	// Only one PCI controller survives: the USB hub (no PCI id) is skipped and
	// the duplicate VEN_8086/DEV_15B8 is deduplicated.
	if len(controllers) != 1 {
		t.Fatalf("got %d controllers, want 1", len(controllers))
	}
	c := controllers[0]
	want := map[string]any{
		"VENDORID":       "8086",
		"PRODUCTID":      "15b8",
		"NAME":           "Intel(R) Ethernet Connection I219-V",
		"MANUFACTURER":   "Intel",
		"CAPTION":        "Intel(R) Ethernet Connection",
		"TYPE":           "Intel(R) Ethernet Connection",
		"PCISUBSYSTEMID": "8086:0000", // SUBSYS_00008086 -> grp2:grp1
	}
	for k, v := range want {
		if c[k] != v {
			t.Errorf("controller[%s] = %v, want %v", k, c[k], v)
		}
	}
}

// TestBuildWinControllersEmpty checks a list with no PCI devices yields none.
func TestBuildWinControllersEmpty(t *testing.T) {
	objs := []map[string]any{{"Name": "Generic", "DeviceID": "ROOT\\SYSTEM\\0000"}}
	if c := buildWinControllers(objs); len(c) != 0 {
		t.Errorf("expected no controllers, got %v", c)
	}
}
