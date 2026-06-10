// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func loadCIMArray(t *testing.T, name string) []map[string]any {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "wincim", name))
	if err != nil {
		t.Fatal(err)
	}
	objs, err := decodeCIMJSON(data)
	if err != nil {
		t.Fatal(err)
	}
	return objs
}

// TestBuildWinCPUs checks the Win32_Processor mapping (cores/threads, vendor
// canonicalisation, whitespace-stripped serial).
func TestBuildWinCPUs(t *testing.T) {
	cpus := buildWinCPUs(loadCIMArray(t, "win32_processor.json"))
	if len(cpus) != 1 {
		t.Fatalf("got %d cpus, want 1", len(cpus))
	}
	c := cpus[0]
	want := map[string]any{
		"NAME":         "Intel(R) Core(TM) i7-10700 CPU @ 2.90GHz",
		"DESCRIPTION":  "Intel64 Family 6 Model 165 Stepping 5",
		"MANUFACTURER": "Intel",     // GenuineIntel -> Intel
		"SERIAL":       "BSN123456", // whitespace stripped
		"ID":           "BFEBFBFF000906EA",
		"CORE":         8,
		"THREAD":       2, // 16 logical / 8 cores
		"SPEED":        2600,
	}
	for k, v := range want {
		if c[k] != v {
			t.Errorf("cpu[%s] = %v, want %v", k, c[k], v)
		}
	}
}

// TestBuildWinMemories checks the Win32_PhysicalMemory mapping (capacity byte->MB,
// the FormFactor/MemoryType enum tables, slot numbering).
func TestBuildWinMemories(t *testing.T) {
	mems := buildWinMemories(loadCIMArray(t, "win32_physicalmemory.json"))
	if len(mems) != 2 {
		t.Fatalf("got %d memories, want 2", len(mems))
	}
	m0 := mems[0]
	want := map[string]any{
		"NUMSLOTS":     1,
		"CAPACITY":     16384, // 17179869184 / 1MiB
		"CAPTION":      "BANK 0",
		"FORMFACTOR":   "DIMM", // FormFactor 8
		"REMOVABLE":    0,
		"SPEED":        2933,
		"TYPE":         "DDR", // MemoryType 20
		"SERIALNUMBER": "SN0001",
	}
	for k, v := range want {
		if m0[k] != v {
			t.Errorf("mem0[%s] = %v, want %v", k, m0[k], v)
		}
	}
	if mems[1]["NUMSLOTS"] != 2 || mems[1]["SERIALNUMBER"] != "SN0002" {
		t.Errorf("mem1 = %v", mems[1])
	}
}

// TestEnumAt covers the bounds handling of the enum tables.
func TestEnumAt(t *testing.T) {
	if enumAt(winFormFactorVal, 8) != "DIMM" {
		t.Error("index 8 should be DIMM")
	}
	if enumAt(winMemoryTypeVal, 99) != "" {
		t.Error("out-of-range index should be empty")
	}
}
