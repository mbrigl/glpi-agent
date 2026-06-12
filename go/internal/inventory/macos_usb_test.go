// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func loadIOReg(t *testing.T, name string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "ioreg", name))
	if err != nil {
		t.Fatalf("read ioreg %s: %v", name, err)
	}
	return string(data)
}

// TestBuildMacUSB pins the ioreg USB parser against the real captures, using the
// expected values from t/tasks/inventory/macos/usb.t.
func TestBuildMacUSB(t *testing.T) {
	usb := buildMacUSB(parseIORegDevices(loadIOReg(t, "IOUSBDevice1"), "IOUSBDevice"))
	if len(usb) != 5 {
		t.Fatalf("IOUSBDevice1: got %d usb devices, want 5", len(usb))
	}

	byName := map[string]map[string]any{}
	for _, u := range usb {
		byName[u["NAME"].(string)] = u
	}

	kbd := byName["Apple Internal Keyboard / Trackpad"]
	if kbd["VENDORID"] != "0x5ac" || kbd["PRODUCTID"] != "0x21b" ||
		kbd["CLASS"] != "0" || kbd["SUBCLASS"] != "0" {
		t.Errorf("keyboard = %v", kbd)
	}
	if _, ok := kbd["SERIAL"]; ok {
		t.Errorf("keyboard should have no SERIAL")
	}

	// Flash Disk carries a serial.
	flash := byName["Flash Disk"]
	if flash["SERIAL"] != "16270078C5C90000" || flash["VENDORID"] != "0x1976" || flash["PRODUCTID"] != "0x6025" {
		t.Errorf("flash disk = %v", flash)
	}

	// Bluetooth controller: NAME comes from "USB Product Name", not the node name.
	bt := byName["Bluetooth USB Host Controller"]
	if bt["CLASS"] != "224" || bt["SUBCLASS"] != "1" || bt["PRODUCTID"] != "0x8205" {
		t.Errorf("bluetooth = %v", bt)
	}

	// Second capture: 6 devices, incl. serials.
	usb2 := buildMacUSB(parseIORegDevices(loadIOReg(t, "IOUSBDevice2"), "IOUSBDevice"))
	if len(usb2) != 6 {
		t.Fatalf("IOUSBDevice2: got %d usb devices, want 6", len(usb2))
	}
	var isight map[string]any
	for _, u := range usb2 {
		if u["NAME"] == "Built-in iSight" {
			isight = u
		}
	}
	if isight["SERIAL"] != "6067E773DA9722F4 (03.01)" || isight["CLASS"] != "239" || isight["SUBCLASS"] != "2" {
		t.Errorf("iSight = %v", isight)
	}
}

// TestMacDec2Hex covers the decimal->hex conversion.
func TestMacDec2Hex(t *testing.T) {
	cases := map[string]string{
		"1452":  "0x5ac",
		"539":   "0x21b",
		"0x5ac": "0x5ac",
		"":      "",
	}
	for in, want := range cases {
		if got := macDec2Hex(in); got != want {
			t.Errorf("macDec2Hex(%q) = %q, want %q", in, got, want)
		}
	}
}
