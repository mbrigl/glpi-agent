// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinInputs checks keyboards + pointing devices map to INPUTS with the
// LAYOUT / POINTINGTYPE / decoded INTERFACE fields and NAME dedupe.
func TestBuildWinInputs(t *testing.T) {
	kbd := []map[string]any{
		{"Name": "Standard PS/2 Keyboard", "Manufacturer": "(Standard keyboards)", "Layout": "00000409"},
		{"Name": "Standard PS/2 Keyboard", "Layout": "dup"}, // dedupe by NAME
	}
	ptr := []map[string]any{
		{"Name": "HID-compliant mouse", "PointingType": float64(4), "DeviceInterface": float64(162)},
	}
	in := buildWinInputs(kbd, ptr)
	if len(in) != 2 {
		t.Fatalf("got %d inputs, want 2", len(in))
	}
	if in[0]["LAYOUT"] != "00000409" || in[0]["MANUFACTURER"] != "(Standard keyboards)" {
		t.Errorf("keyboard = %v", in[0])
	}
	if in[1]["INTERFACE"] != "USB" || in[1]["POINTINGTYPE"] != "4" {
		t.Errorf("mouse = %v", in[1])
	}
}

// TestBuildWinModems checks the Win32_POTSModem -> MODEMS mapping.
func TestBuildWinModems(t *testing.T) {
	objs := []map[string]any{
		{"Name": "Standard Modem", "DeviceType": "External", "Model": "M1", "Description": "desc"},
		{"DeviceType": "noname"}, // skipped, no NAME
	}
	m := buildWinModems(objs)
	if len(m) != 1 {
		t.Fatalf("got %d modems, want 1", len(m))
	}
	if m[0]["TYPE"] != "External" || m[0]["MODEL"] != "M1" {
		t.Errorf("modem = %v", m[0])
	}
}

// TestWinChassis covers the ChassisTypes decode (array and scalar forms).
func TestWinChassis(t *testing.T) {
	if got := winChassis(map[string]any{"ChassisTypes": []any{float64(10)}}); got != "Notebook" {
		t.Errorf("array form = %q, want Notebook", got)
	}
	if got := winChassis(map[string]any{"ChassisTypes": "9"}); got != "Laptop" {
		t.Errorf("scalar form = %q, want Laptop", got)
	}
	if got := winChassis(nil); got != "" {
		t.Errorf("nil = %q, want empty", got)
	}
	if got := winChassis(map[string]any{"ChassisTypes": []any{float64(999)}}); got != "" {
		t.Errorf("out-of-range = %q, want empty", got)
	}
}
