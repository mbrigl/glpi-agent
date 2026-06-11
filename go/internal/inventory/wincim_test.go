// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

// TestBuildWinOS feeds a representative Win32_OperatingSystem CIM-JSON object
// (the shape `Get-CimInstance | ConvertTo-Json` produces) to the parser and
// checks the OPERATINGSYSTEM mapping of Win32/OS.pm.
func TestBuildWinOS(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("testdata", "wincim", "win32_operatingsystem.json"))
	if err != nil {
		t.Fatal(err)
	}
	objs, err := decodeCIMJSON(data)
	if err != nil {
		t.Fatal(err)
	}
	if len(objs) != 1 {
		t.Fatalf("decoded %d objects, want 1", len(objs))
	}

	osSection := buildWinOS(objs[0])
	want := map[string]any{
		"NAME":           "Windows",
		"ARCH":           "64-bit",
		"KERNEL_VERSION": "10.0.22631",
		"FULL_NAME":      "Microsoft Windows 11 Pro",
		"BOOT_TIME":      "2024-01-15 08:30:00",
		"INSTALL_DATE":   "2023-06-01 12:00:00",
	}
	for k, v := range want {
		if osSection[k] != v {
			t.Errorf("OS[%s] = %v, want %v", k, osSection[k], v)
		}
	}
	// CSDVersion was null -> no SERVICE_PACK key.
	if _, ok := osSection["SERVICE_PACK"]; ok {
		t.Errorf("SERVICE_PACK should be absent, got %v", osSection["SERVICE_PACK"])
	}
}

// TestDecodeCIMJSON checks the single-object and array normalisation.
func TestDecodeCIMJSON(t *testing.T) {
	single, _ := decodeCIMJSON([]byte(`{"Caption":"X"}`))
	if len(single) != 1 || single[0]["Caption"] != "X" {
		t.Errorf("single = %v", single)
	}
	many, _ := decodeCIMJSON([]byte(`[{"A":1},{"A":2}]`))
	if len(many) != 2 {
		t.Errorf("array = %v", many)
	}
	if empty, err := decodeCIMJSON([]byte("  ")); err != nil || empty != nil {
		t.Errorf("empty = %v (err %v)", empty, err)
	}
}

// TestWMIDateTime covers the CIM_DATETIME formatting across the raw WMI format,
// the canonical pass-through, and the PowerShell ConvertTo-Json serialisations
// (ISO-8601 with "T" and the Microsoft "/Date(ms±HHMM)/" form).
func TestWMIDateTime(t *testing.T) {
	cases := map[string]string{
		"20240115083000.500000+060":    "2024-01-15 08:30:00",
		"2024-01-15 08:30:00":          "2024-01-15 08:30:00", // already canonical
		"2024-01-15T08:30:00":          "2024-01-15 08:30:00", // ISO-8601, no tz
		"2024-01-15T08:30:00+01:00":    "2024-01-15 08:30:00", // ISO-8601 + offset
		"2024-01-15T08:30:00.5000000Z": "2024-01-15 08:30:00", // ISO-8601 + fraction + Z
		`/Date(1705305000000+0100)/`:   "2024-01-15 08:50:00", // MS JSON (07:50 UTC + 01:00)
		"":                             "",
		"garbage":                      "",
	}
	for in, want := range cases {
		if got := wmiDateTime(in); got != want {
			t.Errorf("wmiDateTime(%q) = %q, want %q", in, got, want)
		}
	}

	// The offset-less "/Date(ms)/" form is rendered in the host's local zone;
	// assert it round-trips for the local interpretation of the epoch ms.
	const ms = int64(1705305000000)
	if got, want := wmiDateTime(`/Date(1705305000000)/`),
		time.UnixMilli(ms).Local().Format("2006-01-02 15:04:05"); got != want {
		t.Errorf("local /Date()/ = %q, want %q", got, want)
	}
}

// TestWinArch covers the architecture mapping.
func TestWinArch(t *testing.T) {
	cases := map[string]string{
		"64-bit": "64-bit",
		"32-bit": "32-bit",
		"ARM 64": "Arm64",
		"arm64":  "Arm64",
		"":       "32-bit",
	}
	for in, want := range cases {
		if got := winArch(in); got != want {
			t.Errorf("winArch(%q) = %q, want %q", in, got, want)
		}
	}
}
