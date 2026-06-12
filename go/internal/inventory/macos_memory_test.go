// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildMacMemories pins the memory mapper against the real captures, using
// the expected values from t/tasks/inventory/macos/memory.t.
func TestBuildMacMemories(t *testing.T) {
	// 10.4 PowerPC: 8 DIMM slots (2 populated DDR2, 6 empty).
	old := loadSP(t, "10.4-powerpc")
	mems := buildMacMemories(old)
	if len(mems) != 8 {
		t.Fatalf("10.4: got %d memories, want 8", len(mems))
	}
	if mems[0]["NUMSLOTS"] != "0" || mems[0]["TYPE"] != "DDR2 SDRAM" ||
		mems[0]["CAPACITY"] != 1024 || mems[0]["CAPTION"] != "Status: OK" {
		t.Errorf("10.4 mem[0] = %v", mems[0])
	}
	// An empty slot: TYPE "Empty", no CAPACITY.
	if mems[2]["TYPE"] != "Empty" || mems[2]["CAPTION"] != "Status: Empty" {
		t.Errorf("10.4 mem[2] = %v", mems[2])
	}
	if _, ok := mems[2]["CAPACITY"]; ok {
		t.Errorf("10.4 empty slot should have no CAPACITY")
	}

	// 11.0 Apple M1: integrated memory fallback.
	m1 := loadSP(t, "11.0-apple-M1")
	memsM1 := buildMacMemories(m1)
	if len(memsM1) != 1 {
		t.Fatalf("M1: got %d memories, want 1", len(memsM1))
	}
	want := map[string]any{
		"NUMSLOTS":    "0",
		"DESCRIPTION": "Integrated memory",
		"TYPE":        "LPDDR4",
		"CAPACITY":    16384,
	}
	for k, v := range want {
		if memsM1[0][k] != v {
			t.Errorf("M1 mem[%s] = %v, want %v", k, memsM1[0][k], v)
		}
	}

	if total := macTotalMemoryMB(m1); total != 16384 {
		t.Errorf("M1 total memory = %d, want 16384", total)
	}
}
