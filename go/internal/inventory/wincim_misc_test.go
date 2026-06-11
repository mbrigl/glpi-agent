// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinPrinters checks the Win32_Printer mapping, status enum, flags and
// resolution, with entries sorted by name.
func TestBuildWinPrinters(t *testing.T) {
	printers := buildWinPrinters(loadCIMArray(t, "win32_printer.json"))
	if len(printers) != 2 {
		t.Fatalf("got %d printers, want 2", len(printers))
	}
	// Sorted by lower-case name -> "HP LaserJet Pro" first.
	hp := printers[0]
	want := map[string]any{
		"NAME":           "HP LaserJet Pro",
		"DRIVER":         "HP LaserJet Pro M404",
		"PORT":           "USB001",
		"NETWORK":        0,
		"SHARED":         1,
		"STATUS":         "Idle", // PrinterStatus 3
		"PRINTPROCESSOR": "winprint",
		"COMMENT":        "Office printer",
		"SHARENAME":      "HPLJ",
		"RESOLUTION":     "1200x1200",
	}
	for k, v := range want {
		if hp[k] != v {
			t.Errorf("printer[%s] = %v, want %v", k, hp[k], v)
		}
	}
}

// TestBuildWinProcesses checks the Win32_Process mapping (PID, CMD fallback,
// STARTED date).
func TestBuildWinProcesses(t *testing.T) {
	procs := buildWinProcesses(loadCIMArray(t, "win32_process.json"))
	if len(procs) != 2 {
		t.Fatalf("got %d processes, want 2", len(procs))
	}
	if procs[0]["PID"] != 4567 || procs[0]["CMD"] != `C:\Windows\Explorer.EXE` || procs[0]["STARTED"] != "2024-01-15 08:30:00" {
		t.Errorf("proc0 = %v", procs[0])
	}
	// System process: no CommandLine -> CMD falls back to Name.
	if procs[1]["CMD"] != "System" || procs[1]["USER"] != "System" {
		t.Errorf("proc1 CMD/USER = %v / %v", procs[1]["CMD"], procs[1]["USER"])
	}
}
