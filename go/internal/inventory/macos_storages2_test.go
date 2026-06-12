// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildMacDiscBurningStorages pins the optical-drive mapper against the real
// capture, using the storages.t expected values.
func TestBuildMacDiscBurningStorages(t *testing.T) {
	disc := buildMacDiscBurningStorages(loadPlist(t, "SPDiscBurningDataType.xml"))
	if len(disc) != 1 {
		t.Fatalf("got %d disc-burning storages, want 1", len(disc))
	}
	want := map[string]any{
		"NAME":         "OPTIARC DVD RW AD-5630A",
		"MANUFACTURER": "Sony",
		"INTERFACE":    "ATAPI",
		"MODEL":        "OPTIARC DVD RW AD-5630A",
		"FIRMWARE":     "1AHN",
		"TYPE":         "Disk burning",
	}
	for k, v := range want {
		if disc[0][k] != v {
			t.Errorf("disc[%s] = %v, want %v", k, disc[0][k], v)
		}
	}
}

// TestBuildMacCardReaderStorages pins the card-reader mapper, incl. the inserted
// SD card case.
func TestBuildMacCardReaderStorages(t *testing.T) {
	reader := buildMacCardReaderStorages(loadPlist(t, "SPCardReaderDataType.xml"))
	if len(reader) != 1 {
		t.Fatalf("got %d card-reader storages, want 1", len(reader))
	}
	want := map[string]any{
		"NAME":         "spcardreader",
		"SERIAL":       "000000000820",
		"MODEL":        "spcardreader",
		"FIRMWARE":     "3.00",
		"MANUFACTURER": "0x05ac",
		"TYPE":         "Card reader",
		"DESCRIPTION":  "spcardreader",
	}
	for k, v := range want {
		if reader[0][k] != v {
			t.Errorf("reader[%s] = %v, want %v", k, reader[0][k], v)
		}
	}

	// With an inserted card: the reader plus the SD card (sorted: disk2 first).
	withCard := buildMacCardReaderStorages(loadPlist(t, "SPCardReaderDataType_with_inserted_card.xml"))
	if len(withCard) != 2 {
		t.Fatalf("got %d storages with card, want 2", len(withCard))
	}
	var sd map[string]any
	for _, s := range withCard {
		if s["TYPE"] == "SD Card" {
			sd = s
		}
	}
	if sd["NAME"] != "disk2" || sd["DESCRIPTION"] != "SDHC Card" || sd["DISKSIZE"] != 15193 {
		t.Errorf("SD card = %v", sd)
	}
}
