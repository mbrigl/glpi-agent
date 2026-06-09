// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// dmidecodeFixtures lists the real upstream dmidecode captures vendored under
// testdata/dmidecode and the record counts the parser must recover from each
// (type 17 memory devices, type 9 slots, type 8 port connectors), measured
// directly from the fixtures. This pins the Go parser against the same
// real-world inputs the Perl agent is tested on.
var dmidecodeFixtures = []struct {
	name                   string
	memories, slots, ports int
}{
	{"dell-r640", 24, 10, 10},
	{"hp-dl360-gen8", 24, 2, 11},
	{"lenovo-thinkpad", 4, 6, 24},
	{"linux-2.6", 2, 2, 7},
	{"freebsd-8.1", 2, 8, 0}, // edge case: no port connectors
	{"vmware-esx", 15, 7, 4}, // a virtual machine
	{"windows-7", 4, 4, 21},
	{"rhel-5.6", 18, 4, 13},
	{"sun-x2200-m2", 16, 2, 17},
	{"openbsd-4.5", 4, 6, 9},
}

func TestDmidecodeRealFixtures(t *testing.T) {
	for _, f := range dmidecodeFixtures {
		t.Run(f.name, func(t *testing.T) {
			file, err := os.Open(filepath.Join("testdata", "dmidecode", f.name))
			if err != nil {
				t.Fatal(err)
			}
			defer file.Close()

			byType := ParseDmidecode(file)
			if len(byType) == 0 {
				t.Fatal("parser returned no records for a real dmidecode capture")
			}

			if got := len(BuildMemories(byType)); got != f.memories {
				t.Errorf("memories = %d, want %d", got, f.memories)
			}
			if got := len(BuildSlots(byType)); got != f.slots {
				t.Errorf("slots = %d, want %d", got, f.slots)
			}
			if got := len(BuildPorts(byType)); got != f.ports {
				t.Errorf("ports = %d, want %d", got, f.ports)
			}

			// Every memory entry carries its 1-based slot number.
			for i, m := range BuildMemories(byType) {
				if m["NUMSLOTS"] != i+1 {
					t.Errorf("memory %d NUMSLOTS = %v, want %d", i, m["NUMSLOTS"], i+1)
				}
			}
		})
	}
}

// TestDmidecodeExactValues pins the exact fields the parser extracts from the
// first populated DIMM of the dell-r640 capture.
func TestDmidecodeExactValues(t *testing.T) {
	file, err := os.Open(filepath.Join("testdata", "dmidecode", "dell-r640"))
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	mem := BuildMemories(ParseDmidecode(file))[0]
	checks := map[string]any{
		"CAPTION":      "A1",
		"TYPE":         "DDR4",
		"CAPACITY":     32 * 1024, // "32 GB" -> MB
		"SPEED":        2933,
		"MANUFACTURER": "00CE00B300CE",
		"SERIALNUMBER": "3780385B",
	}
	for k, want := range checks {
		if mem[k] != want {
			t.Errorf("memory[0][%s] = %v, want %v", k, mem[k], want)
		}
	}
}
