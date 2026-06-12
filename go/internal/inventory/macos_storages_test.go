// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func loadPlist(t *testing.T, name string) any {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "plist", name))
	if err != nil {
		t.Fatalf("read plist %s: %v", name, err)
	}
	root, err := parsePlist(data)
	if err != nil {
		t.Fatalf("parse plist %s: %v", name, err)
	}
	return root
}

// TestParsePlist checks the plist parser navigates the dict/array structure.
func TestParsePlist(t *testing.T) {
	root := loadPlist(t, "SPUSBDataType.xml")
	items := plistDictArray(root, "")
	if len(items) == 0 {
		t.Fatal("no _items in SPUSBDataType plist")
	}
	first, ok := items[0].(map[string]any)
	if !ok {
		t.Fatal("first item is not a dict")
	}
	if first["_name"] != "USBBus" {
		t.Errorf("first USB item _name = %v, want USBBus", first["_name"])
	}
}

// TestBuildMacATAStorages pins the SATA storage mapper against the real captures,
// using the expected values from t/tasks/inventory/macos/storages.t.
func TestBuildMacATAStorages(t *testing.T) {
	wd := buildMacATAStorages(loadPlist(t, "SPSerialATADataType.xml"), "SATA", true)
	if len(wd) != 1 {
		t.Fatalf("SATA1: got %d storages, want 1", len(wd))
	}
	want := map[string]any{
		"NAME":         "disk0",
		"MANUFACTURER": "Western Digital",
		"INTERFACE":    "SATA",
		"SERIAL":       "WD-WCARY1264478",
		"MODEL":        "WDC WD2500AAJS-40VWA1",
		"FIRMWARE":     "58.01D02",
		"DISKSIZE":     238475,
		"TYPE":         "Disk drive",
		"DESCRIPTION":  "WDC WD2500AAJS-40VWA1",
	}
	for k, v := range want {
		if wd[0][k] != v {
			t.Errorf("SATA1[%s] = %v, want %v", k, wd[0][k], v)
		}
	}

	// SATA2: Apple SSD — manufacturer stripped from the model.
	apple := buildMacATAStorages(loadPlist(t, "SPSerialATADataType2.xml"), "SATA", true)
	if len(apple) != 1 {
		t.Fatalf("SATA2: got %d storages, want 1", len(apple))
	}
	if apple[0]["MANUFACTURER"] != "Apple" || apple[0]["MODEL"] != "SSD SD0128F" ||
		apple[0]["SERIAL"] != "1435NL400611" {
		t.Errorf("SATA2 storage = %v", apple[0])
	}
}

// TestGetCanonicalManufacturer covers the brand normalisation.
func TestGetCanonicalManufacturer(t *testing.T) {
	cases := map[string]string{
		"WDC WD2500AAJS-40VWA1": "Western Digital",
		"APPLE SSD SD0128F":     "Apple",
		"GenuineIntel":          "Intel",
		"SAMSUNG SSD 970":       "Samsung",
		"ST1000DM003":           "Seagate",
		"Unknown Brand XYZ":     "Unknown Brand XYZ",
	}
	for in, want := range cases {
		if got := getCanonicalManufacturer(in); got != want {
			t.Errorf("getCanonicalManufacturer(%q) = %q, want %q", in, got, want)
		}
	}
}
