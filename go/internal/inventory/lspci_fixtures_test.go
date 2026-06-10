// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// lspciFixtures lists the real upstream `lspci -nn` captures vendored under
// testdata/lspci and the number of PCI devices each contains (lines carrying a
// [vendor:device] id, which is what ParseLspci matches). Pins the parser against
// the same real inputs the Perl agent is tested on.
var lspciFixtures = []struct {
	name    string
	devices int
}{
	{"dell-xt2", 24},
	{"linux-2", 19},
	{"linux-imac", 27},
	{"linux-xps", 20},
	{"nvidia-1", 51},
}

func TestLspciRealFixtures(t *testing.T) {
	for _, f := range lspciFixtures {
		t.Run(f.name, func(t *testing.T) {
			file, err := os.Open(filepath.Join("testdata", "lspci", f.name))
			if err != nil {
				t.Fatal(err)
			}
			defer file.Close()

			devices := ParseLspci(file)
			if len(devices) != f.devices {
				t.Errorf("parsed %d devices, want %d", len(devices), f.devices)
			}
			// Controllers are 1:1 with parsed devices.
			if got := len(BuildControllers(devices)); got != len(devices) {
				t.Errorf("controllers = %d, want %d (1:1 with devices)", got, len(devices))
			}
			// Every machine has at least one GPU and one audio device.
			if len(BuildVideos(devices)) == 0 {
				t.Error("expected at least one video device")
			}
			if len(BuildSounds(devices)) == 0 {
				t.Error("expected at least one sound device")
			}
		})
	}
}

// TestLspciExactValues pins the dell-xt2 integrated GPU fields, including the
// trailing "(prog-if ...)" annotation the header line carries.
func TestLspciExactValues(t *testing.T) {
	file, err := os.Open(filepath.Join("testdata", "lspci", "dell-xt2"))
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	videos := BuildVideos(ParseLspci(file))
	if len(videos) == 0 {
		t.Fatal("no videos parsed from dell-xt2")
	}
	// First video is the Intel integrated graphics at 00:02.0.
	v := videos[0]
	if v["PCIID"] != "8086:2a42" {
		t.Errorf("video PCIID = %v, want 8086:2a42", v["PCIID"])
	}
	if v["PCISLOT"] != "00:02.0" {
		t.Errorf("video PCISLOT = %v, want 00:02.0", v["PCISLOT"])
	}
}
