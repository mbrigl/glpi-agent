// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildMacBattery pins the battery mapper against the real capture, using the
// expected values from t/tasks/inventory/macos/batteries.t.
func TestBuildMacBattery(t *testing.T) {
	power := loadSP(t, "10.11-system_profiler_SPPowerDataType.txt")
	battery := buildMacBattery(power)
	want := map[string]any{
		"SERIAL":       "C01437408B3F90MA2",
		"CAPACITY":     "6078",
		"NAME":         "bq20z451",
		"MANUFACTURER": "DP",
		"VOLTAGE":      "7921",
	}
	for k, v := range want {
		if battery[k] != v {
			t.Errorf("battery[%s] = %v, want %v", k, battery[k], v)
		}
	}
}

// TestMacHostname checks the Computer Name extraction.
func TestMacHostname(t *testing.T) {
	m1 := loadSP(t, "11.0-apple-M1")
	if got := macHostname(m1); got != "MacBook Air de test" {
		t.Errorf("macHostname = %q, want 'MacBook Air de test'", got)
	}
}

// TestBuildMacSounds checks the "Audio (Built In)" mapping with a synthetic node
// (the real fixtures use the newer "Audio > Devices" layout the module ignores).
func TestBuildMacSounds(t *testing.T) {
	audio := map[string]any{
		"Audio (Built In)": map[string]any{
			"Built-in Output":     map[string]any{},
			"Built-in Line Input": map[string]any{},
		},
	}
	sounds := buildMacSounds(audio)
	if len(sounds) != 2 {
		t.Fatalf("got %d sounds, want 2", len(sounds))
	}
	// Sorted by name: "Built-in Line Input" first.
	if sounds[0]["NAME"] != "Built-in Line Input" || sounds[0]["MANUFACTURER"] != "Built-in Line Input" ||
		sounds[0]["DESCRIPTION"] != "Built-in Line Input" {
		t.Errorf("sound[0] = %v", sounds[0])
	}
}
