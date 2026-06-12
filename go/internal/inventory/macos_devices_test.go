// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildMacBios pins the BIOS mapper against the real captures.
func TestBuildMacBios(t *testing.T) {
	macmini := spNode(loadSP(t, "10.6-macmini"), "Hardware", "Hardware Overview")
	bios := buildMacBios(macmini, nil)
	want := map[string]any{
		"SMANUFACTURER": "Apple Inc",
		"SMODEL":        "Macmini3,1",
		"SSN":           "YM008ASN9G5",
		"BVERSION":      "MM31.00AD.B00",
	}
	for k, v := range want {
		if bios[k] != v {
			t.Errorf("macmini bios[%s] = %v, want %v", k, bios[k], v)
		}
	}

	// ioreg manufacturer overrides the "Apple Inc" default.
	m1 := spNode(loadSP(t, "11.0-apple-M1"), "Hardware", "Hardware Overview")
	biosM1 := buildMacBios(m1, map[string]any{"manufacturer": "Apple Computer"})
	if biosM1["SMANUFACTURER"] != "Apple Computer" || biosM1["SMODEL"] != "MacBookAir10,1" ||
		biosM1["SSN"] != "E9O1Q6W5FAKE" {
		t.Errorf("M1 bios = %v", biosM1)
	}
}

// TestBuildMacCharger pins the PSU mapper against the psu.t fixtures.
func TestBuildMacCharger(t *testing.T) {
	charging := buildMacCharger(loadSP(t, "charging-SPPowerDataType"))
	want := map[string]any{
		"SERIALNUMBER": "HD2J66XBVX2K",
		"NAME":         "61W USB-C Power Adapter",
		"MANUFACTURER": "Apple Inc.",
		"PLUGGED":      "Yes",
		"STATUS":       "Charging",
		"POWER_MAX":    "60",
	}
	for k, v := range want {
		if charging[k] != v {
			t.Errorf("charging psu[%s] = %v, want %v", k, charging[k], v)
		}
	}

	charged := buildMacCharger(loadSP(t, "charged-SPPowerDataType"))
	if charged["STATUS"] != "Not charging" || charged["PLUGGED"] != "Yes" {
		t.Errorf("charged psu STATUS/PLUGGED = %v/%v", charged["STATUS"], charged["PLUGGED"])
	}
}

// TestBuildMacVideos pins the VIDEOS mapper against the videos.t fixtures.
func TestBuildMacVideos(t *testing.T) {
	// Single card with an attached display.
	asus := buildMacVideos(loadSP(t, "asus-geforce-gt-730"))
	if len(asus) != 1 {
		t.Fatalf("asus: got %d videos, want 1", len(asus))
	}
	want := map[string]any{
		"NAME":       "Asus GeForce GT 730",
		"CHIPSET":    "Asus GeForce GT 730",
		"MEMORY":     1024,
		"PCISLOT":    "PCIe",
		"RESOLUTION": "1920x1080",
	}
	for k, v := range want {
		if asus[0][k] != v {
			t.Errorf("asus video[%s] = %v, want %v", k, asus[0][k], v)
		}
	}

	// Dual cards: Intel (no display) then AMD (with display), sorted by name.
	dual := buildMacVideos(loadSP(t, "dual-display-#475"))
	if len(dual) != 2 {
		t.Fatalf("dual: got %d videos, want 2", len(dual))
	}
	if dual[0]["NAME"] != "Intel HD Graphics 530" || dual[0]["MEMORY"] != 1536 || dual[0]["PCISLOT"] != "Built-In" {
		t.Errorf("dual[0] = %v", dual[0])
	}
	if _, ok := dual[0]["RESOLUTION"]; ok {
		t.Errorf("Intel card should have no RESOLUTION")
	}
	if dual[1]["NAME"] != "Radeon Pro 450" || dual[1]["CHIPSET"] != "AMD Radeon Pro 450" ||
		dual[1]["MEMORY"] != 2048 || dual[1]["RESOLUTION"] != "2880x1800" || dual[1]["PCISLOT"] != "PCIe" {
		t.Errorf("dual[1] = %v", dual[1])
	}
}
