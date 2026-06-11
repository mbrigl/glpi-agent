// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinVideos checks the Win32_VideoController mapping (resolution,
// AdapterRAM byte->MiB, NAME dedupe).
func TestBuildWinVideos(t *testing.T) {
	videos := buildWinVideos(loadCIMArray(t, "win32_videocontroller.json"))
	if len(videos) != 1 { // the second entry is a duplicate NAME
		t.Fatalf("got %d videos, want 1", len(videos))
	}
	v := videos[0]
	want := map[string]any{
		"NAME":       "Intel(R) UHD Graphics 770",
		"CHIPSET":    "Intel(R) UHD Graphics 770",
		"MEMORY":     1024, // 1 GiB
		"RESOLUTION": "2560x1440",
	}
	for k, val := range want {
		if v[k] != val {
			t.Errorf("video[%s] = %v, want %v", k, v[k], val)
		}
	}
}

// TestBuildWinSounds checks the Win32_SoundDevice mapping.
func TestBuildWinSounds(t *testing.T) {
	sounds := buildWinSounds(loadCIMArray(t, "win32_sounddevice.json"))
	if len(sounds) != 1 {
		t.Fatalf("got %d sounds, want 1", len(sounds))
	}
	s := sounds[0]
	want := map[string]any{
		"NAME":         "Realtek High Definition Audio",
		"CAPTION":      "Realtek HD Audio",
		"MANUFACTURER": "Realtek",
		"DESCRIPTION":  "Realtek(R) Audio",
	}
	for k, v := range want {
		if s[k] != v {
			t.Errorf("sound[%s] = %v, want %v", k, s[k], v)
		}
	}
}

// TestBuildWinSlots checks the Win32_SystemSlot mapping and the usage-less skip.
func TestBuildWinSlots(t *testing.T) {
	slots := buildWinSlots(loadCIMArray(t, "win32_systemslot.json"))
	if len(slots) != 2 { // the CurrentUsage=null slot is skipped
		t.Fatalf("got %d slots, want 2", len(slots))
	}
	if slots[0]["NAME"] != "PCIe Slot 1" || slots[0]["DESIGNATION"] != "PCIEX16_1" || slots[0]["STATUS"] != "used" {
		t.Errorf("slot0 = %v", slots[0])
	}
	if slots[1]["STATUS"] != "free" { // CurrentUsage 3
		t.Errorf("slot1 STATUS = %v, want free", slots[1]["STATUS"])
	}
}

// TestBuildWinPorts checks the serial/parallel port TYPE tagging.
func TestBuildWinPorts(t *testing.T) {
	ports := buildWinPorts(loadCIMArray(t, "win32_serialport.json"), "Serial")
	if len(ports) != 1 {
		t.Fatalf("got %d ports, want 1", len(ports))
	}
	p := ports[0]
	if p["NAME"] != "COM1" || p["CAPTION"] != "Communications Port (COM1)" || p["TYPE"] != "Serial" {
		t.Errorf("port = %v", p)
	}
}
