// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// writeFile creates path under root with the given content.
func writeFile(t *testing.T, root, rel, content string) {
	t.Helper()
	full := filepath.Join(root, rel)
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestBuildStorages(t *testing.T) {
	root := t.TempDir()
	// A real disk with a device/ subdir.
	writeFile(t, root, "sys/block/sda/size", "1953525168\n")
	writeFile(t, root, "sys/block/sda/device/model", "Samsung SSD 970\n")
	writeFile(t, root, "sys/block/sda/device/vendor", "ATA\n") // dropped
	writeFile(t, root, "sys/block/sda/device/rev", "1B2QEXM7\n")
	// A loop device with no device/ subdir -> ignored.
	writeFile(t, root, "sys/block/loop0/size", "0\n")

	storages := BuildStorages(root)
	if len(storages) != 1 {
		t.Fatalf("got %d storages, want 1 (loop ignored)", len(storages))
	}
	s := storages[0]
	if s["NAME"] != "sda" || s["TYPE"] != "disk" || s["MODEL"] != "Samsung SSD 970" {
		t.Errorf("storage = %v", s)
	}
	if s["FIRMWARE"] != "1B2QEXM7" {
		t.Errorf("FIRMWARE = %v", s["FIRMWARE"])
	}
	if _, present := s["MANUFACTURER"]; present {
		t.Errorf("MANUFACTURER should be dropped for ATA: %v", s["MANUFACTURER"])
	}
	// 1953525168 sectors * 512 / 1MiB
	if s["DISKSIZE"] != int(1953525168*512/(1024*1024)) {
		t.Errorf("DISKSIZE = %v", s["DISKSIZE"])
	}
}

func TestBuildBatteries(t *testing.T) {
	root := t.TempDir()
	base := "sys/class/power_supply/BAT0/"
	writeFile(t, root, base+"type", "Battery\n")
	writeFile(t, root, base+"present", "1\n")
	writeFile(t, root, base+"capacity", "95\n")
	writeFile(t, root, base+"model_name", "DELL ABC123\n")
	writeFile(t, root, base+"technology", "Li-ion\n")
	writeFile(t, root, base+"serial_number", "0001\n")
	writeFile(t, root, base+"manufacturer", "Samsung SDI\n")
	writeFile(t, root, base+"voltage_min_design", "11400000\n") // µV
	writeFile(t, root, base+"energy_full_design", "68010000\n") // µWh
	// A mains supply that must be skipped.
	writeFile(t, root, "sys/class/power_supply/AC/type", "Mains\n")

	bats := BuildBatteries(root)
	if len(bats) != 1 {
		t.Fatalf("got %d batteries, want 1 (Mains skipped)", len(bats))
	}
	b := bats[0]
	if b["NAME"] != "DELL ABC123" || b["CHEMISTRY"] != "Li-ion" || b["SERIAL"] != "0001" {
		t.Errorf("battery = %v", b)
	}
	if b["VOLTAGE"] != 11400 { // µV -> mV
		t.Errorf("VOLTAGE = %v, want 11400", b["VOLTAGE"])
	}
	if b["CAPACITY"] != 68010 { // µWh -> mWh
		t.Errorf("CAPACITY = %v, want 68010", b["CAPACITY"])
	}
}
