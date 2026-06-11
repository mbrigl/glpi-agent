// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinFirewall checks all three profiles are emitted in the upstream
// order with the STATUS derived from the EnableFirewall flag.
func TestBuildWinFirewall(t *testing.T) {
	fw := buildWinFirewall(map[string]bool{"domain": true, "public": false})
	if len(fw) != 3 {
		t.Fatalf("got %d firewall profiles, want 3", len(fw))
	}
	want := []struct {
		profile, status string
	}{
		{"DomainProfile", "on"},
		{"PublicProfile", "off"},
		{"StandardProfile", "off"}, // absent from the map -> off
	}
	for i, w := range want {
		if fw[i]["PROFILE"] != w.profile || fw[i]["STATUS"] != w.status {
			t.Errorf("profile[%d] = %v, want %s/%s", i, fw[i], w.profile, w.status)
		}
	}
}
