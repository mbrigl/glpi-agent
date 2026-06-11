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
