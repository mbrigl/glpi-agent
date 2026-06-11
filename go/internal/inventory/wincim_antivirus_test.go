// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinAntivirus checks the AntiVirusProduct -> ANTIVIRUS mapping: the
// COMPANY/NAME/GUID/VERSION passthrough, the productState hex decode of
// ENABLED/UPTODATE, the boolean fallback when productState is absent, and the
// NAME+VERSION dedupe across namespaces.
func TestBuildWinAntivirus(t *testing.T) {
	objs := []map[string]any{
		{
			"companyName":             "Microsoft Corporation",
			"displayName":             "Windows Defender",
			"instanceGuid":            "{D68DDC3A-831F-4fae-9E44-DA132C1ACF46}",
			"versionNumber":           "4.18.2205.7",
			"onAccessScanningEnabled": false,
			"productUptoDate":         false,
			// 0x61100 -> last4 "1100" -> enabled "11" (->1), uptodate "00" (->1)
			"productState": float64(397568),
		},
		// Disabled + out of date: 0x60900 -> last4 "0900" -> enabled "09" (->0),
		// uptodate "00" (->1). Different NAME so kept.
		{
			"displayName":  "ACME AV",
			"productState": float64(396032),
		},
		// No productState -> falls back to the booleans.
		{
			"displayName":             "Bool AV",
			"onAccessScanningEnabled": true,
			"productUptoDate":         true,
		},
		// Duplicate of the first (same NAME+VERSION) -> deduped.
		{
			"displayName":   "Windows Defender",
			"versionNumber": "4.18.2205.7",
			"productState":  float64(397568),
		},
	}

	av := buildWinAntivirus(objs)
	if len(av) != 3 {
		t.Fatalf("got %d antivirus entries, want 3 (dedupe by NAME+VERSION)", len(av))
	}

	def := av[0]
	want := map[string]any{
		"NAME":     "Windows Defender",
		"COMPANY":  "Microsoft Corporation",
		"GUID":     "{D68DDC3A-831F-4fae-9E44-DA132C1ACF46}",
		"VERSION":  "4.18.2205.7",
		"ENABLED":  1,
		"UPTODATE": 1,
	}
	for k, v := range want {
		if def[k] != v {
			t.Errorf("defender[%s] = %v, want %v", k, def[k], v)
		}
	}

	if av[1]["ENABLED"] != 0 || av[1]["UPTODATE"] != 1 {
		t.Errorf("acme ENABLED/UPTODATE = %v/%v, want 0/1", av[1]["ENABLED"], av[1]["UPTODATE"])
	}

	// Boolean fallback: no productState, so the booleans drive the flags.
	if av[2]["ENABLED"] != 1 || av[2]["UPTODATE"] != 1 {
		t.Errorf("bool AV ENABLED/UPTODATE = %v/%v, want 1/1", av[2]["ENABLED"], av[2]["UPTODATE"])
	}
}

// TestDecodeProductState covers the trailing byte-pair decode and the too-short
// guard.
func TestDecodeProductState(t *testing.T) {
	cases := []struct {
		state             int
		enabled, uptodate int
		ok                bool
	}{
		{397568, 1, 1, true}, // 0x61100
		{396032, 0, 1, true}, // 0x60900
		{266240, 1, 1, true}, // 0x41000 -> "1000" enabled "10", uptodate "00"
		{6, 0, 0, false},     // 0x6 -> too short
	}
	for _, c := range cases {
		e, u, ok := decodeProductState(c.state)
		if ok != c.ok || (ok && (e != c.enabled || u != c.uptodate)) {
			t.Errorf("decodeProductState(%d) = (%d,%d,%v), want (%d,%d,%v)",
				c.state, e, u, ok, c.enabled, c.uptodate, c.ok)
		}
	}
}

// TestBuildWinEnvironment checks the Win32_Environment -> ENVS mapping keeps only
// system variables.
func TestBuildWinEnvironment(t *testing.T) {
	objs := []map[string]any{
		{"SystemVariable": true, "Name": "Path", "VariableValue": `C:\Windows`},
		{"SystemVariable": "1", "Name": "TEMP", "VariableValue": `C:\Temp`},
		// User variable (SystemVariable false) -> skipped.
		{"SystemVariable": false, "Name": "USERVAR", "VariableValue": "x"},
		// Missing name -> skipped.
		{"SystemVariable": "1", "VariableValue": "y"},
	}
	envs := buildWinEnvironment(objs)
	if len(envs) != 2 {
		t.Fatalf("got %d envs, want 2", len(envs))
	}
	if envs[0]["KEY"] != "Path" || envs[0]["VAL"] != `C:\Windows` {
		t.Errorf("env[0] = %v", envs[0])
	}
	if envs[1]["KEY"] != "TEMP" {
		t.Errorf("env[1] KEY = %v, want TEMP", envs[1]["KEY"])
	}
}
