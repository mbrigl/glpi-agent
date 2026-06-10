// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinDrives checks the Win32_LogicalDisk mapping (sizes byte->MiB,
// DriveType enum, system-drive flag).
func TestBuildWinDrives(t *testing.T) {
	drives := buildWinDrives(loadCIMArray(t, "win32_logicaldisk.json"), "C:")
	if len(drives) != 2 {
		t.Fatalf("got %d drives, want 2", len(drives))
	}
	c := drives[0]
	want := map[string]any{
		"DESCRIPTION": "Local Fixed Disk",
		"FILESYSTEM":  "NTFS",
		"LABEL":       "Windows",
		"VOLUMN":      "Windows",
		"LETTER":      "C:",
		"SERIAL":      "A1B2C3D4",
		"TYPE":        "Local Disk", // DriveType 3
		"FREE":        117737,       // 123456789012 / 1MiB (truncated)
		"TOTAL":       488281,       // 512000000000 / 1MiB
		"SYSTEMDRIVE": 1,
		"CREATEDATE":  "2023-06-01 12:00:00",
	}
	for k, v := range want {
		if c[k] != v {
			t.Errorf("drive C[%s] = %v, want %v", k, c[k], v)
		}
	}
	// The CD-ROM (DriveType 5) has no size and is not the system drive.
	cd := drives[1]
	if cd["TYPE"] != "Compact Disc" || cd["LETTER"] != "D:" {
		t.Errorf("drive D = %v", cd)
	}
	if _, ok := cd["SYSTEMDRIVE"]; ok {
		t.Errorf("D: should not be the system drive")
	}
}

// TestBuildWinStorages checks the Win32_DiskDrive mapping (decimal MB size,
// trimmed firmware, first-token serial).
func TestBuildWinStorages(t *testing.T) {
	storages := buildWinStorages(loadCIMArray(t, "win32_diskdrive.json"))
	if len(storages) != 1 {
		t.Fatalf("got %d storages, want 1", len(storages))
	}
	st := storages[0]
	want := map[string]any{
		"MANUFACTURER": "(Standard disk drives)",
		"MODEL":        "Samsung SSD 980 1TB",
		"DESCRIPTION":  "Disk drive",
		"NAME":         `\\.\PHYSICALDRIVE0`,
		"TYPE":         "Fixed hard disk media",
		"INTERFACE":    "SCSI",
		"FIRMWARE":     "1B2QGXA7",       // trailing space trimmed
		"DISKSIZE":     1000204,          // 1000204886016 / 1_000_000
		"SERIAL":       "S5GXNX0R123456", // first token, trimmed
	}
	for k, v := range want {
		if st[k] != v {
			t.Errorf("storage[%s] = %v, want %v", k, st[k], v)
		}
	}
}
