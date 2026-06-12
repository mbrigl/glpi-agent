// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func loadAV(t *testing.T, name string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "antivirus", name))
	if err != nil {
		t.Fatalf("read av %s: %v", name, err)
	}
	return string(data)
}

// TestBuildMacDefender pins the Defender JSON parser against the real fixture
// (the antivirus.t expected values). Dates are checked in the local zone the
// upstream localtime() also uses.
func TestBuildMacDefender(t *testing.T) {
	av := buildMacDefender([]byte(loadAV(t, "defender-101.98.30.json")))
	want := map[string]any{
		"COMPANY":       "Microsoft",
		"NAME":          "Microsoft Defender",
		"ENABLED":       1,
		"UPTODATE":      1,
		"VERSION":       "101.98.30",
		"BASE_VERSION":  "1.389.10.0",
		"EXPIRATION":    time.Unix(1693977148000/1000, 0).Local().Format("2006-01-02"),
		"BASE_CREATION": time.Unix(1683092938089/1000, 0).Local().Format("2006-01-02"),
	}
	for k, v := range want {
		if av[k] != v {
			t.Errorf("defender[%s] = %v, want %v", k, av[k], v)
		}
	}
}

// TestBuildMacCortex pins the Cortex detector against the three real fixtures.
func TestBuildMacCortex(t *testing.T) {
	av := buildMacCortex(
		loadAV(t, "cortex-xdr-8.2.1.47908-info"),
		loadAV(t, "cortex-xdr-8.2.1.47908-info-query"),
		loadAV(t, "cortex-xdr-8.2.1.47908-runtime-query"),
	)
	want := map[string]any{
		"COMPANY":      "Palo Alto Networks",
		"NAME":         "Cortex XDR",
		"ENABLED":      1,
		"VERSION":      "8.2.1.47908",
		"BASE_VERSION": "1270-79108",
	}
	for k, v := range want {
		if av[k] != v {
			t.Errorf("cortex[%s] = %v, want %v", k, av[k], v)
		}
	}
}

// TestBuildMacSentinelOne pins the SentinelOne detector against the real fixtures.
func TestBuildMacSentinelOne(t *testing.T) {
	av := buildMacSentinelOne(
		loadAV(t, "sentinelone-epp-24.1.2.7444-version"),
		loadAV(t, "sentinelone-epp-24.1.2.7444-status"),
	)
	want := map[string]any{
		"COMPANY": "Sentinel Labs Inc.",
		"NAME":    "SentinelOne EPP",
		"ENABLED": 1,
		"VERSION": "24.1.2.7444",
	}
	for k, v := range want {
		if av[k] != v {
			t.Errorf("sentinelone[%s] = %v, want %v", k, av[k], v)
		}
	}
}
