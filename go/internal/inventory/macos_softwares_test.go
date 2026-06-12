// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// findSoftware returns the software entry with the given NAME.
func findSoftware(list []map[string]any, name string) map[string]any {
	for _, s := range list {
		if s["NAME"] == name {
			return s
		}
	}
	return nil
}

// TestBuildMacSoftwaresText pins the text-format SPApplicationsDataType path
// against the real sample1 capture (count from softwares.t + spot-checks from
// the .results.txt dump).
func TestBuildMacSoftwaresText(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "system_profiler", "sample1.SPApplicationsDataType"))
	if err != nil {
		t.Fatalf("read sample1: %v", err)
	}
	apps := extractMacSoftwaresFromText(parseSystemProfiler(string(data)))
	sw := buildMacSoftwares(apps)
	if len(sw) != 349 {
		t.Fatalf("sample1: got %d softwares, want 349", len(sw))
	}

	// Non-Apple system app: no PUBLISHER, swapped MM/DD/YYYY install date.
	palette := findSoftware(sw, "50onPaletteServer")
	if palette["ARCH"] != "Universal" || palette["VERSION"] != "1.0.3" ||
		palette["INSTALLDATE"] != "06/30/2009" || palette["SYSTEM_CATEGORY"] != "System/Library" {
		t.Errorf("50onPaletteServer = %v", palette)
	}
	if _, ok := palette["PUBLISHER"]; ok {
		t.Errorf("50onPaletteServer should have no PUBLISHER")
	}
	// CoreServices location -> PUBLISHER Apple.
	ard := findSoftware(sw, "ARDAgent")
	if ard["PUBLISHER"] != "Apple" || ard["VERSION"] != "3.5.2" || ard["INSTALLDATE"] != "02/17/2012" {
		t.Errorf("ARDAgent = %v", ard)
	}
}

// TestBuildMacSoftwaresXML pins the XML-format path against the real sample5
// capture.
func TestBuildMacSoftwaresXML(t *testing.T) {
	root := loadPlist(t, "sample5.SPApplicationsDataType-xml")
	sw := buildMacSoftwares(extractMacSoftwaresFromXML(root, 7200))
	if len(sw) != 367 {
		t.Fatalf("sample5: got %d softwares, want 367", len(sw))
	}

	// XML path: DD/MM/YYYY install date from lastModified + the fixed offset.
	palette := findSoftware(sw, "50onPaletteServer")
	if palette["ARCH"] != "Universal" || palette["VERSION"] != "1.1.0" ||
		palette["INSTALLDATE"] != "26/02/2024" || palette["SYSTEM_CATEGORY"] != "System/Library" {
		t.Errorf("50onPaletteServer (xml) = %v", palette)
	}

	// Identified-developer publisher cleanup.
	aam := findSoftware(sw, "AAM Registration Notifier")
	if aam["PUBLISHER"] != "Adobe Systems Inc." {
		t.Errorf("AAM Registration Notifier PUBLISHER = %v, want Adobe Systems Inc.", aam["PUBLISHER"])
	}
}

// TestMacFormatDate / TestMacOffsetDate cover the two date paths.
func TestMacFormatDate(t *testing.T) {
	if got := macFormatDate("30/06/09 07:29"); got != "06/30/2009" {
		t.Errorf("macFormatDate = %q, want 06/30/2009", got)
	}
	if got := macFormatDate("not a date"); got != "not a date" {
		t.Errorf("macFormatDate passthrough = %q", got)
	}
}

func TestMacOffsetDate(t *testing.T) {
	// 2024-02-26T23:30:00Z + 7200s -> still 27/02 in UTC after the shift.
	if got := macOffsetDate("2024-02-26T23:30:00Z", 7200); got != "27/02/2024" {
		t.Errorf("macOffsetDate = %q, want 27/02/2024", got)
	}
	if got := macOffsetDate("2024-02-26T10:00:00Z", 7200); got != "26/02/2024" {
		t.Errorf("macOffsetDate = %q, want 26/02/2024", got)
	}
}
