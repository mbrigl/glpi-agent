// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinSoftwares checks the Uninstall->SOFTWARES mapping: value mapping,
// version control-char strip, install date, DWORD fields, SYSTEM_CATEGORY, the
// single-value skip and the NAME+ARCH+VERSION dedupe.
func TestBuildWinSoftwares(t *testing.T) {
	entries := []winSoftwareEntry{
		{Key: "{GUID-1}", Values: map[string]string{
			"DisplayName":     "Mozilla Firefox",
			"DisplayVersion":  "126.0\x00garbage",
			"Publisher":       "Mozilla",
			"Comments":        "Firefox browser",
			"UninstallString": `"C:\Program Files\Mozilla Firefox\uninstall\helper.exe"`,
			"InstallDate":     "20240515",
			"MajorVersion":    "126",
			"MinorVersion":    "0",
			"SystemComponent": "0",
		}},
		{Key: "{GUID-2}", Values: map[string]string{
			"DisplayName":     "Windows Driver Package",
			"DisplayVersion":  "1.0",
			"SystemComponent": "1",
		}},
		// Single value -> skipped.
		{Key: "{GUID-noise}", Values: map[string]string{"InstallSource": "x"}},
		// Duplicate of GUID-1 (same NAME+ARCH+VERSION) -> deduped.
		{Key: "{GUID-1-dup}", Values: map[string]string{"DisplayName": "Mozilla Firefox", "DisplayVersion": "126.0"}},
	}

	sw := buildWinSoftwares(entries, "x86_64")
	if len(sw) != 2 {
		t.Fatalf("got %d softwares, want 2 (skip single-value + dedupe)", len(sw))
	}

	ff := sw[0]
	want := map[string]any{
		"FROM":             "registry",
		"NAME":             "Mozilla Firefox",
		"ARCH":             "x86_64",
		"GUID":             "{GUID-1}",
		"VERSION":          "126.0", // control char and trailing garbage stripped
		"PUBLISHER":        "Mozilla",
		"COMMENTS":         "Firefox browser",
		"INSTALLDATE":      "15/05/2024",
		"VERSION_MAJOR":    126,
		"VERSION_MINOR":    0,
		"SYSTEM_CATEGORY":  "application",
		"UNINSTALL_STRING": `"C:\Program Files\Mozilla Firefox\uninstall\helper.exe"`,
	}
	for k, v := range want {
		if ff[k] != v {
			t.Errorf("firefox[%s] = %v, want %v", k, ff[k], v)
		}
	}

	// The driver package is flagged as a system component.
	if sw[1]["SYSTEM_CATEGORY"] != "system_component" {
		t.Errorf("driver SYSTEM_CATEGORY = %v, want system_component", sw[1]["SYSTEM_CATEGORY"])
	}
}

// TestWinSoftwareDate covers the InstallDate formats.
func TestWinSoftwareDate(t *testing.T) {
	cases := map[string]string{
		"20240515": "15/05/2024",
		"2024515":  "15/05/2024", // single-digit month padded
		"":         "",
		"weird":    "weird",
	}
	for in, want := range cases {
		if got := winSoftwareDate(in); got != want {
			t.Errorf("winSoftwareDate(%q) = %q, want %q", in, got, want)
		}
	}
}

// TestHex2Dec covers the DWORD parsing.
func TestHex2Dec(t *testing.T) {
	if n, ok := hex2dec("0x1a"); !ok || n != 26 {
		t.Errorf("hex = %d %v", n, ok)
	}
	if n, ok := hex2dec("126"); !ok || n != 126 {
		t.Errorf("dec = %d %v", n, ok)
	}
	if _, ok := hex2dec(""); ok {
		t.Error("empty should be ok=false")
	}
}
