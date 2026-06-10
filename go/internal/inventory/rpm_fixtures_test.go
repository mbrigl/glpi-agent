// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

// TestRPMRealFixture replays the real upstream rpm package list (the tab-separated
// `rpm -qa --qf` output GLPI uses) and pins the parsed count and the first
// package's fields.
func TestRPMRealFixture(t *testing.T) {
	out, err := os.ReadFile(filepath.Join("testdata", "packaging", "rpm"))
	if err != nil {
		t.Fatal(err)
	}
	pkgs := ParseRPMQA(string(out))
	if len(pkgs) != 10 {
		t.Fatalf("parsed %d packages, want 10", len(pkgs))
	}

	first := pkgs[0]
	want := map[string]any{
		"NAME":            "libpciaccess0",
		"FROM":            "rpm",
		"ARCH":            "i586",
		"VERSION":         "0.12.1-1.mga1",
		"FILESIZE":        38452,
		"PUBLISHER":       "Mageia.Org",
		"COMMENTS":        "Generic PCI access library (from X.org)",
		"SYSTEM_CATEGORY": "System Environment/Libraries",
	}
	for k, v := range want {
		if first[k] != v {
			t.Errorf("pkg[0][%s] = %v, want %v", k, first[k], v)
		}
	}
	// INSTALLDATE comes from the epoch in column 4 (1311080703), formatted
	// dd/mm/yyyy in local time (asserted with the same formula to stay TZ-stable).
	if got, exp := first["INSTALLDATE"], time.Unix(1311080703, 0).Format("02/01/2006"); got != exp {
		t.Errorf("INSTALLDATE = %v, want %v", got, exp)
	}

	// Spot-check a later package to confirm row independence.
	if pkgs[2]["NAME"] != "gjs" || pkgs[2]["VERSION"] != "1.32.0-1.mga2" || pkgs[2]["ARCH"] != "x86_64" {
		t.Errorf("pkg[2] = %v", pkgs[2])
	}
}
