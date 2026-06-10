// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func readFixture(t *testing.T, parts ...string) string {
	t.Helper()
	b, err := os.ReadFile(filepath.Join(append([]string{"testdata"}, parts...)...))
	if err != nil {
		t.Fatal(err)
	}
	return string(b)
}

// TestBitdefenderRealFixture replays the real bduitool capture and pins the
// ANTIVIRUS fields against t/.../antivirus/bitdefender.t.
func TestBitdefenderRealFixture(t *testing.T) {
	av := ParseBitdefender(readFixture(t, "antivirus", "bduitool-7.0.3.2239"))
	want := map[string]any{
		"COMPANY":       "Bitdefender",
		"NAME":          "Bitdefender Endpoint Security Tools (BEST) for Linux",
		"ENABLED":       1,
		"UPTODATE":      1,
		"VERSION":       "7.0.3.2239",
		"BASE_VERSION":  "7.95171",
		"BASE_CREATION": "2023-08-24",
	}
	for k, v := range want {
		if av[k] != v {
			t.Errorf("bitdefender[%s] = %v, want %v", k, av[k], v)
		}
	}
}

// TestSentinelOneRealFixture replays the real sentinelctl capture.
func TestSentinelOneRealFixture(t *testing.T) {
	av := ParseSentinelOne(readFixture(t, "antivirus", "sentinelone-30.1.1.10"))
	want := map[string]any{
		"COMPANY":      "SentinelOne",
		"NAME":         "SentinelAgent",
		"ENABLED":      1,
		"UPTODATE":     1,
		"VERSION":      "30.1.1.10",
		"BASE_VERSION": "30.5.6.5",
	}
	for k, v := range want {
		if av[k] != v {
			t.Errorf("sentinelone[%s] = %v, want %v", k, av[k], v)
		}
	}
}

// TestTeamViewerRealFixture replays the real teamviewer-info capture and pins the
// extracted ID against t/.../remote_mgmt/teamviewer.t ('15.65.4-DEB' => "552").
func TestTeamViewerRealFixture(t *testing.T) {
	mgmt := ParseTeamViewerInfo(readFixture(t, "teamviewer", "teamviewer-info-15.65.4-DEB"))
	if mgmt == nil {
		t.Fatal("no TeamViewer entry parsed")
	}
	if mgmt["ID"] != "552" {
		t.Errorf("ID = %v, want 552", mgmt["ID"])
	}
	if mgmt["TYPE"] != "teamviewer" {
		t.Errorf("TYPE = %v, want teamviewer", mgmt["TYPE"])
	}
}
