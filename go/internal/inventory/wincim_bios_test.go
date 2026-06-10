// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// loadCIM decodes the first object from a CIM-JSON fixture.
func loadCIM(t *testing.T, name string) map[string]any {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "wincim", name))
	if err != nil {
		t.Fatal(err)
	}
	objs, err := decodeCIMJSON(data)
	if err != nil || len(objs) == 0 {
		t.Fatalf("decode %s: %v (%d objs)", name, err, len(objs))
	}
	return objs[0]
}

// TestBuildWinBios checks the BIOS mapping across the four CIM classes,
// including the SSN priority (Win32_Bios serial wins) and the date/trim/invalid
// cleanups.
func TestBuildWinBios(t *testing.T) {
	b := buildWinBios(
		loadCIM(t, "win32_bios.json"),
		loadCIM(t, "win32_computersystem.json"),
		loadCIM(t, "win32_systemenclosure.json"),
		loadCIM(t, "win32_baseboard.json"),
	)
	want := map[string]any{
		"BIOSSERIAL":      "BIOSSN123", // trailing spaces trimmed
		"SSN":             "BIOSSN123", // Win32_Bios serial wins the SSN chain
		"BMANUFACTURER":   "Dell Inc.",
		"BVERSION":        "1.21.0", // SMBIOSBIOSVersion preferred
		"BDATE":           "06/01/2023",
		"SMANUFACTURER":   "Dell Inc.",
		"SMODEL":          "Latitude 7440",
		"ENCLOSURESERIAL": "ENC9988",
		"ASSETTAG":        "ASSET-42",
		"MSN":             "BB-7777",
		"MMODEL":          "0ABCD",
		"MMANUFACTURER":   "Dell Inc.",
	}
	for k, v := range want {
		if b[k] != v {
			t.Errorf("bios[%s] = %v, want %v", k, b[k], v)
		}
	}
}

// TestBuildWinHardware checks the HARDWARE mapping (memory/swap byte->MB, UUID,
// owner/workgroup fallbacks).
func TestBuildWinHardware(t *testing.T) {
	h := buildWinHardware(
		loadCIM(t, "win32_operatingsystem.json"),
		loadCIM(t, "win32_computersystem.json"),
		loadCIM(t, "win32_computersystemproduct.json"),
	)
	want := map[string]any{
		"NAME":       "desktop-ab12", // DNSHostName preferred over Name
		"UUID":       "4C4C4544-0042-4210-8030-C2C04F313233",
		"WINLANG":    "1033",
		"WINPRODID":  "00330-80000-00000-AA123",
		"WINCOMPANY": "ACME Corp",
		"WINOWNER":   "Alice",
		"WORKGROUP":  "WORKGROUP", // Domain preferred (Workgroup null)
		"MEMORY":     16384,       // 17179869184 / 1MiB
		"SWAP":       4096,        // 4294967296 / 1MiB
	}
	for k, v := range want {
		if h[k] != v {
			t.Errorf("hardware[%s] = %v, want %v", k, h[k], v)
		}
	}
}

// TestBuildWinHardwareDropsPlaceholderUUID checks an all-zero UUID is dropped.
func TestBuildWinHardwareDropsPlaceholderUUID(t *testing.T) {
	h := buildWinHardware(nil, nil, map[string]any{"UUID": "00000000-0000-0000-0000-000000000000"})
	if _, ok := h["UUID"]; ok {
		t.Errorf("placeholder UUID should be dropped, got %v", h["UUID"])
	}
}
