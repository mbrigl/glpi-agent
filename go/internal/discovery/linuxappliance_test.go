// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// laGetter builds a fakeGetter with the linux net-snmp sysObjectID that selects
// the LinuxAppliance module, merging extra scalar values and walks.
func laGetter(values map[string]string, walks map[string]map[string]string) *fakeGetter {
	base := map[string]string{
		oidSysDescr:    "Linux appliance 5.10",
		oidSysObjectID: ".1.3.6.1.4.1.8072.3.2.10",
	}
	for k, v := range values {
		base[k] = v
	}
	return &fakeGetter{values: base, walks: walks}
}

// TestLinuxApplianceSynology checks the Synology detection, serial, firmware and
// the disk/volume enrichment in the run hook.
func TestLinuxApplianceSynology(t *testing.T) {
	getter := laGetter(map[string]string{
		laDsmModelName:    "DS920+",
		laDsmSerialNumber: "SYNO-SN-1",
		laDsmVersion:      "DSM 7.1",
	}, map[string]map[string]string{
		laSynoDiskModel:     {"0": "WD40EFRX", "1": "ST4000VN008"},
		laSynoDiskName:      {"0": "Disk 1", "1": "Disk 2"},
		laSynoDiskType:      {"0": "SATA", "1": "SATA"},
		laSynoRaidName:      {"0": "Volume 1"},
		laSynoRaidFreeSize:  {"0": "1000000000"}, // 1000 MB
		laSynoRaidTotalSize: {"0": "4000000000"}, // 4000 MB
	})
	d, err := GetInventory("192.0.2.100", getter)
	if err != nil {
		t.Fatal(err)
	}
	if d["TYPE"] != "STORAGE" || d["MANUFACTURER"] != "Synology" || d["MODEL"] != "DS920+" {
		t.Errorf("identity = %v", d)
	}
	if d["SERIAL"] != "SYNO-SN-1" {
		t.Errorf("SERIAL = %v", d["SERIAL"])
	}
	if _, ok := d["_appliance"]; ok {
		t.Error("_appliance scratch key leaked into output")
	}
	storages, _ := d["STORAGES"].([]map[string]any)
	if len(storages) != 2 {
		t.Fatalf("STORAGES = %v", storages)
	}
	// Manufacturer derived from the disk model prefixes.
	manus := map[string]bool{}
	for _, s := range storages {
		manus[s["MANUFACTURER"].(string)] = true
	}
	if !manus["Western Digital"] || !manus["Seagate"] {
		t.Errorf("disk manufacturers = %v", manus)
	}
	drives, _ := d["DRIVES"].([]map[string]any)
	if len(drives) != 1 || drives[0]["FREE"] != 1000 || drives[0]["TOTAL"] != 4000 {
		t.Errorf("DRIVES = %v", drives)
	}
	fws, _ := d["FIRMWARES"].([]map[string]any)
	if len(fws) != 1 || fws[0]["VERSION"] != "DSM 7.1" {
		t.Errorf("FIRMWARES = %v", fws)
	}
}

// TestLinuxApplianceCheckPoint checks the CheckPoint detection + serial + SVN
// firmware.
func TestLinuxApplianceCheckPoint(t *testing.T) {
	getter := laGetter(map[string]string{
		laSvnApplianceManu: "Check Point",
		laSvnApplianceMod:  "T-180",
		laSvnApplianceSN:   "CP-SN-9",
		laSvnProdName:      "Check Point SVN",
		laSvnVersion:       "R81.10",
	}, nil)
	d, _ := GetInventory("192.0.2.101", getter)
	if d["TYPE"] != "NETWORKING" || d["MANUFACTURER"] != "CheckPoint" || d["MODEL"] != "T-180" {
		t.Errorf("identity = %v", d)
	}
	if d["SERIAL"] != "CP-SN-9" {
		t.Errorf("SERIAL = %v", d["SERIAL"])
	}
	fws, _ := d["FIRMWARES"].([]map[string]any)
	if len(fws) != 1 || fws[0]["VERSION"] != "R81.10" {
		t.Errorf("FIRMWARES = %v", fws)
	}
}

// TestLinuxApplianceTplinkSysDescr checks the TP-Link sysDescr fallback.
func TestLinuxApplianceTplinkSysDescr(t *testing.T) {
	getter := laGetter(nil, nil)
	getter.values[oidSysDescr] = "Linux TL-SG3210 3.10.0 #1 SMP"
	d, _ := GetInventory("192.0.2.102", getter)
	if d["MANUFACTURER"] != "TP-Link" || d["MODEL"] != "TL-SG3210" {
		t.Errorf("tplink = %v", d)
	}
}

// TestLinuxApplianceEngineID checks the snmpEngineID manufacturer decode and the
// Veritas NetBackup installed-software refinement. Manufacturer id 9 = Cisco.
func TestLinuxApplianceEngineID(t *testing.T) {
	// engineID first byte 0x80 (high bit set) + manufacturer id 9 in bytes 1..3
	// -> 0x80 00 00 09, then format byte and payload (ignored here).
	getter := laGetter(map[string]string{
		laSnmpEngineID: "80:00:00:09:04:61:62:63",
	}, map[string]map[string]string{
		laHrSWInstalledName: {"1": "VRTSnbserver-10.0"},
	})
	d, _ := GetInventory("192.0.2.103", getter)
	if d["MANUFACTURER"] != "Veritas Technologies LLC" || d["MODEL"] != "Veritas NetBackup" {
		t.Errorf("engineID detect = %v", d)
	}
}

// TestLinuxApplianceNoMatch verifies a plain Linux host keeps its generic
// sysObjectID classification (Net-SNMP) and leaks no scratch state.
func TestLinuxApplianceNoMatch(t *testing.T) {
	getter := laGetter(nil, nil)
	d, _ := GetInventory("192.0.2.104", getter)
	if d["MANUFACTURER"] != "Net-SNMP" {
		t.Errorf("expected the generic Net-SNMP classification, got %v", d["MANUFACTURER"])
	}
	if _, ok := d["_appliance"]; ok {
		t.Error("_appliance scratch key leaked into output")
	}
}

// TestCanonicalDiskManufacturer covers a few disk-model prefixes.
func TestCanonicalDiskManufacturer(t *testing.T) {
	cases := map[string]string{
		"WD40EFRX":    "Western Digital",
		"ST4000VN008": "Seagate",
		"HGST HUS726": "Hitachi",
		"Samsung SSD": "Samsung",
		"CT500MX500":  "Crucial",
		"unknownXYZ":  "",
	}
	for model, want := range cases {
		if got := canonicalDiskManufacturer(model); got != want {
			t.Errorf("%q -> %q, want %q", model, got, want)
		}
	}
}
