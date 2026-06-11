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
// STARTED date, and the GetOwner USER rules: local-domain strip, "@domain"
// append, NT AUTHORITY strip, Name fallback).
func TestBuildWinProcesses(t *testing.T) {
	procs := buildWinProcesses(loadCIMArray(t, "win32_process.json"), "DESKTOP")
	if len(procs) != 4 {
		t.Fatalf("got %d processes, want 4", len(procs))
	}
	// explorer: owner DESKTOP\jdoe -> local computer domain stripped -> "jdoe".
	if procs[0]["PID"] != 4567 || procs[0]["CMD"] != `C:\Windows\Explorer.EXE` ||
		procs[0]["STARTED"] != "2024-01-15 08:30:00" || procs[0]["USER"] != "jdoe" {
		t.Errorf("proc0 = %v", procs[0])
	}
	// System process: no owner + no CommandLine -> USER/CMD fall back to Name.
	if procs[1]["CMD"] != "System" || procs[1]["USER"] != "System" {
		t.Errorf("proc1 CMD/USER = %v / %v", procs[1]["CMD"], procs[1]["USER"])
	}
	// svc.exe: foreign domain -> "svc@CORP".
	if procs[2]["USER"] != "svc@CORP" {
		t.Errorf("proc2 USER = %v, want svc@CORP", procs[2]["USER"])
	}
	// services.exe: NT AUTHORITY domain stripped -> "SYSTEM".
	if procs[3]["USER"] != "SYSTEM" {
		t.Errorf("proc3 USER = %v, want SYSTEM", procs[3]["USER"])
	}
}

// TestBuildWinLoggedUsers checks that only Explorer.exe owners become logged
// users, deduplicated by LOGIN.
func TestBuildWinLoggedUsers(t *testing.T) {
	users := buildWinLoggedUsers(loadCIMArray(t, "win32_process.json"))
	if len(users) != 1 {
		t.Fatalf("got %d logged users, want 1", len(users))
	}
	if users[0]["LOGIN"] != "jdoe" || users[0]["DOMAIN"] != "DESKTOP" {
		t.Errorf("logged user = %v", users[0])
	}
}

// TestMergeWinUsers checks the last-user-first ordering and the
// lc(LOGIN)@lc(DOMAIN) dedupe.
func TestMergeWinUsers(t *testing.T) {
	last := map[string]any{"LOGIN": "jdoe", "DOMAIN": "DESKTOP"}
	logged := []map[string]any{
		{"LOGIN": "JDoe", "DOMAIN": "desktop"}, // case-insensitive dup of last
		{"LOGIN": "svc", "DOMAIN": "CORP"},
	}
	users := mergeWinUsers(last, logged)
	if len(users) != 2 {
		t.Fatalf("got %d users, want 2 (deduped)", len(users))
	}
	if users[0]["LOGIN"] != "jdoe" || users[1]["LOGIN"] != "svc" {
		t.Errorf("users = %v", users)
	}
}
