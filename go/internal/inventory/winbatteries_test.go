// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// TestParsePowercfgBatteries pins the powercfg parser against the real upstream
// captures in resources/win32/powercfg, using the expected values from
// t/tasks/inventory/windows/batteries.t (_getBatteriesFromPowercfg).
func TestParsePowercfgBatteries(t *testing.T) {
	cases := map[string][]map[string]any{
		"windows-10-notebook": {
			{"NAME": "00HW023", "CAPACITY": 23540, "CHEMISTRY": "LiP", "SERIAL": "541", "MANUFACTURER": "SMP", "REAL_CAPACITY": 19450},
			{"NAME": "01AV406", "CAPACITY": 26060, "CHEMISTRY": "LiP", "SERIAL": "3319", "MANUFACTURER": "SMP", "REAL_CAPACITY": 17860},
		},
		"win10-dell-xps": {
			{"NAME": "DELL JHXPY53", "CAPACITY": 57532, "CHEMISTRY": "LiP", "MANUFACTURER": "SMP", "SERIAL": "2677", "REAL_CAPACITY": 48807},
		},
	}

	for name, want := range cases {
		data, err := os.ReadFile(filepath.Join("testdata", "powercfg", name+".xml"))
		if err != nil {
			t.Fatalf("read fixture %s: %v", name, err)
		}
		got := parsePowercfgBatteries(data)
		if len(got) != len(want) {
			t.Fatalf("%s: got %d batteries, want %d", name, len(got), len(want))
		}
		for i, w := range want {
			for k, v := range w {
				if got[i][k] != v {
					t.Errorf("%s[%d][%s] = %v, want %v", name, i, k, got[i][k], v)
				}
			}
		}
	}
}

// TestSanitizeBatterySerial covers the serial normalisation forms.
func TestSanitizeBatterySerial(t *testing.T) {
	cases := map[string]string{
		"  541":  "541",    // whitespace -> non-hex branch trims
		"2677":   "2677",   // pure decimal hex digits, no a-f, no leading 0 -> as-is
		"0000":   "0",      // zeros only
		"":       "0",      // empty -> 0
		"2A3F":   "10815",  // hex with letters -> decimal
		"01F":    "31",     // leading zero -> treated as hex
		"SN-123": "SN-123", // non-hex -> trimmed passthrough
	}
	for in, want := range cases {
		if got := sanitizeBatterySerial(in); got != want {
			t.Errorf("sanitizeBatterySerial(%q) = %q, want %q", in, got, want)
		}
	}
}
