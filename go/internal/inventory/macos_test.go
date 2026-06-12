// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func loadSP(t *testing.T, name string) map[string]any {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "system_profiler", name))
	if err != nil {
		t.Fatalf("read fixture %s: %v", name, err)
	}
	return parseSystemProfiler(string(data))
}

// TestParseSystemProfiler checks the indentation parser against the real
// system_profiler captures: nested section navigation and leaf values.
func TestParseSystemProfiler(t *testing.T) {
	m1 := loadSP(t, "11.0-apple-M1")
	if got := spString(m1, "Hardware", "Hardware Overview", "Model Name"); got != "MacBook Air" {
		t.Errorf("M1 Model Name = %q, want MacBook Air", got)
	}
	if got := spString(m1, "Hardware", "Hardware Overview", "Chip"); got != "Apple M1" {
		t.Errorf("M1 Chip = %q, want Apple M1", got)
	}
	if got := spString(m1, "Software", "System Software Overview", "System Version"); got != "macOS 11.2.3 (20D91)" {
		t.Errorf("M1 System Version = %q", got)
	}

	old := loadSP(t, "10.4-powerpc")
	if got := spString(old, "Hardware", "Hardware Overview", "Machine Name"); got != "Power Mac G5" {
		t.Errorf("10.4 Machine Name = %q", got)
	}
}

// TestMacSystemVersion covers the name/version split.
func TestMacSystemVersion(t *testing.T) {
	cases := map[string][2]string{
		"macOS 11.2.3 (20D91)":     {"macOS", "11.2.3 (20D91)"},
		"Mac OS X 10.4.11 (8S165)": {"Mac OS X", "10.4.11 (8S165)"},
		"Garbage":                  {"", ""},
	}
	for in, want := range cases {
		name, ver := macSystemVersion(in)
		if name != want[0] || ver != want[1] {
			t.Errorf("macSystemVersion(%q) = %q/%q, want %q/%q", in, name, ver, want[0], want[1])
		}
	}
}

// TestBuildMacOSAndHardware pins the OS + hardware mappers against the fixtures.
func TestBuildMacOSAndHardware(t *testing.T) {
	m1 := loadSP(t, "11.0-apple-M1")
	osSec := buildMacOS(m1)
	if osSec["NAME"] != "MacOSX" || osSec["FULL_NAME"] != "macOS" || osSec["VERSION"] != "11.2.3 (20D91)" {
		t.Errorf("M1 os = %v", osSec)
	}
	hw := buildMacHardware(m1, m1)
	if hw["NAME"] != "macOS" || hw["UUID"] != "2B05150B-FA33-45BD-90F0-3369BF908E3E" {
		t.Errorf("M1 hardware = %v", hw)
	}

	old := loadSP(t, "10.4-powerpc")
	hwOld := buildMacHardware(old, old)
	if hwOld["NAME"] != "Mac OS X" {
		t.Errorf("10.4 hardware NAME = %v", hwOld["NAME"])
	}
	if _, ok := hwOld["UUID"]; ok {
		t.Errorf("10.4 should have no Hardware UUID, got %v", hwOld["UUID"])
	}
}
