// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func readLVM(t *testing.T, kind, name string) string {
	t.Helper()
	b, err := os.ReadFile(filepath.Join("testdata", "lvm", kind, name))
	if err != nil {
		t.Fatal(err)
	}
	return string(b)
}

// TestLVMRealFixtures replays the real upstream pvs/vgs/lvs captures and pins
// the parsed record counts and the first record's fields against them.
func TestLVMRealFixtures(t *testing.T) {
	pvsCounts := map[string]int{"linux-1": 3, "linux-2": 3, "linux-3": 3}
	for name, want := range pvsCounts {
		if got := len(ParsePVS(readLVM(t, "pvs", name))); got != want {
			t.Errorf("pvs/%s = %d PVs, want %d", name, got, want)
		}
	}
	vgsCounts := map[string]int{"linux-1": 2, "linux-2": 1, "linux-3": 2}
	for name, want := range vgsCounts {
		if got := len(ParseVGS(readLVM(t, "vgs", name))); got != want {
			t.Errorf("vgs/%s = %d VGs, want %d", name, got, want)
		}
	}
	if got := len(ParseLVS(readLVM(t, "lvs", "linux-1"))); got != 8 {
		t.Errorf("lvs/linux-1 = %d LVs, want 8", got)
	}

	// Exact fields of the first physical volume (size 15846.08 MB / 3778 PEs).
	// These PVs are not assigned to a VG, so VG_UUID is absent.
	pv := ParsePVS(readLVM(t, "pvs", "linux-1"))[0]
	for k, want := range map[string]any{
		"DEVICE": "/dev/sda5", "FORMAT": "lvm2", "ATTR": "a-",
		"SIZE": 15846, "FREE": 0, "PV_PE_COUNT": "3778",
		"PV_UUID": "MjsnP7-GaGC-NIo7-tS3o-gf2t-di2R-eP3Au7", "PE_SIZE": 4,
	} {
		if pv[k] != want {
			t.Errorf("pv[%s] = %v, want %v", k, pv[k], want)
		}
	}
	if _, ok := pv["VG_UUID"]; ok {
		t.Errorf("pv VG_UUID should be absent for an unassigned PV, got %v", pv["VG_UUID"])
	}

	// Exact fields of the first volume group.
	vg := ParseVGS(readLVM(t, "vgs", "linux-1"))[0]
	for k, want := range map[string]any{
		"VG_NAME": "lvm", "PV_COUNT": "1", "LV_COUNT": "6", "ATTR": "wz--n-",
		"SIZE": 15846, "FREE": 0, "VG_EXTENT_SIZE": "4.19",
	} {
		if vg[k] != want {
			t.Errorf("vg[%s] = %v, want %v", k, vg[k], want)
		}
	}

	// Exact fields of the first logical volume (size 5901.39 MB).
	lv := ParseLVS(readLVM(t, "lvs", "linux-1"))[0]
	for k, want := range map[string]any{
		"LV_NAME": "home", "ATTR": "-wi-ao", "SIZE": 5901, "SEG_COUNT": "1",
	} {
		if lv[k] != want {
			t.Errorf("lv[%s] = %v, want %v", k, lv[k], want)
		}
	}
}
