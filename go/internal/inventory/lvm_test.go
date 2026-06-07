// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestParsePVS(t *testing.T) {
	// Leading-indented output as produced by `pvs --noheading --nosuffix --units M`.
	const out = "  /dev/sda2 lvm2 a-- 476800.00 0.00 pvuuid-1 119200 vguuid-1\n"
	pvs := ParsePVS(out)
	if len(pvs) != 1 {
		t.Fatalf("pvs = %d, want 1", len(pvs))
	}
	p := pvs[0]
	if p["DEVICE"] != "/dev/sda2" || p["FORMAT"] != "lvm2" || p["ATTR"] != "a--" {
		t.Errorf("pv base wrong: %v", p)
	}
	if p["SIZE"] != 476800 || p["FREE"] != 0 || p["PV_UUID"] != "pvuuid-1" || p["VG_UUID"] != "vguuid-1" {
		t.Errorf("pv sizes/uuids wrong: %v", p)
	}
	// PE_SIZE = int(SIZE / PV_PE_COUNT) = 476800 / 119200 = 4.
	if p["PE_SIZE"] != 4 {
		t.Errorf("PE_SIZE = %v, want 4", p["PE_SIZE"])
	}
}

func TestParseVGS(t *testing.T) {
	const out = "  vg0 1 2 wz--n- 476800.00 1024.00 vguuid-1 4.00\n"
	vgs := ParseVGS(out)
	if len(vgs) != 1 {
		t.Fatalf("vgs = %d, want 1", len(vgs))
	}
	g := vgs[0]
	if g["VG_NAME"] != "vg0" || g["PV_COUNT"] != "1" || g["LV_COUNT"] != "2" {
		t.Errorf("vg base wrong: %v", g)
	}
	if g["SIZE"] != 476800 || g["FREE"] != 1024 || g["VG_UUID"] != "vguuid-1" || g["VG_EXTENT_SIZE"] != "4.00" {
		t.Errorf("vg sizes wrong: %v", g)
	}
}

func TestParseLVS(t *testing.T) {
	const out = "  root vguuid-1 -wi-ao---- 51200.00 lvuuid-1 1\n  swap vguuid-1 -wi-ao---- 8192.00 lvuuid-2 1\n"
	lvs := ParseLVS(out)
	if len(lvs) != 2 {
		t.Fatalf("lvs = %d, want 2", len(lvs))
	}
	if lvs[0]["LV_NAME"] != "root" || lvs[0]["SIZE"] != 51200 || lvs[0]["SEG_COUNT"] != "1" {
		t.Errorf("lv0 = %v", lvs[0])
	}
	if lvs[1]["LV_NAME"] != "swap" || lvs[1]["LV_UUID"] != "lvuuid-2" {
		t.Errorf("lv1 = %v", lvs[1])
	}
}
