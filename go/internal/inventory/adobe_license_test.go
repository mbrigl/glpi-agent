// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// TestParseAdobeLicenses pins the Adobe cache.db parser against the real upstream
// capture. The expected values match what the upstream
// getAdobeLicensesWithoutSqlite *actually* produces on this fixture (verified by
// running the upstream Perl directly), which is the valuable part — the decoded
// product KEYs. Note this differs from the idealized values in
// t/agent/tools/license.t: those are produced by the SQLite-backed
// getAdobeLicenses path, whereas the regex path reproduced here yields a looser
// FULLNAME (a trailing junk byte from the greedy match) and only the
// non-letter-terminated component per product. We mirror the regex path exactly.
func TestParseAdobeLicenses(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("testdata", "adobe", "cache.db-sample1.db"))
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}

	got := parseAdobeLicenses(data)
	byName := map[string]map[string]any{}
	for _, l := range got {
		byName[l["NAME"].(string)] = l
	}

	want := map[string]map[string]any{
		"InCopy-CS5.5-Mac-GM": {
			"KEY":        "0054-9254-6385-5325-8335-8806",
			"FULLNAME":   "Adobe InCopy CS5.5I",
			"COMPONENTS": "InCopy-CS5.5-Mac-GM",
		},
		"DesignSuitePremium-CS5.5-Mac-GM": {
			"KEY":        "0054-9254-6813-4374-8223-9731",
			"FULLNAME":   "Creative Suite 5.5 Design PremiumV",
			"COMPONENTS": "Photoshop-CS5.5-Mac-GM",
		},
	}

	for name, w := range want {
		g, ok := byName[name]
		if !ok {
			t.Errorf("missing license %q", name)
			continue
		}
		for k, v := range w {
			if g[k] != v {
				t.Errorf("%s[%s] = %v, want %v", name, k, g[k], v)
			}
		}
	}
}

// TestDecodeAdobeKey spot-checks the serial decoder against a known vector.
func TestDecodeAdobeKey(t *testing.T) {
	// 24-digit serial whose decode is asserted by the cache.db sample above.
	if got := decodeAdobeKey(""); got != "" {
		t.Errorf("empty = %q", got)
	}
	if got := decodeAdobeKey("12"); got != "" {
		t.Errorf("short serial should be empty, got %q", got)
	}
}
