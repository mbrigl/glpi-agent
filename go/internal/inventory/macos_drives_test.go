// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestParseMacDf checks the macOS df parser (no Type column) and the KiB->MiB
// conversion + virtual-filesystem skip.
func TestParseMacDf(t *testing.T) {
	out := `Filesystem    1024-blocks      Used Available Capacity  Mounted on
/dev/disk1s1    488245288 200000000 280000000    42%    /
map -hosts              0         0         0   100%    /net
/dev/disk1s4     10485760   1048576   9437184    10%    /private/var/vm`
	fs := parseMacDf(out, "apfs")
	if len(fs) != 2 {
		t.Fatalf("got %d filesystems, want 2 (skip zero-total /net)", len(fs))
	}
	root := fs[0]
	if root["VOLUMN"] != "/dev/disk1s1" || root["FILESYSTEM"] != "apfs" || root["TYPE"] != "/" {
		t.Errorf("root = %v", root)
	}
	// 488245288 KiB / 1024 = 476802 MiB.
	if root["TOTAL"] != 476802 || root["FREE"] != 273437 {
		t.Errorf("root TOTAL/FREE = %v/%v", root["TOTAL"], root["FREE"])
	}
}

// TestParseMacDiskutil checks the partition list + info parsers.
func TestParseMacDiskutil(t *testing.T) {
	list := `/dev/disk1 (synthesized):
   #:  TYPE NAME       SIZE       IDENTIFIER
   1:  APFS Container  500.3 GB   disk1s1
   2:  APFS Volume     10.5 GB    disk1s4`
	parts := parseMacDiskutilPartitions(list)
	if len(parts) != 2 || parts[0] != "disk1s1" || parts[1] != "disk1s4" {
		t.Errorf("partitions = %v", parts)
	}

	info := parseMacDiskutilInfo(`   Volume Name:              Macintosh HD
   Total Size:               500.3 GB (500277792768 Bytes)
   Volume UUID:              ABCD-1234
   File System:              APFS`)
	if info["Volume Name"] != "Macintosh HD" || info["Volume UUID"] != "ABCD-1234" ||
		info["File System"] != "APFS" {
		t.Errorf("info = %v", info)
	}
}

// TestBuildMacDrives checks the df+diskutil join + FileVault.
func TestBuildMacDrives(t *testing.T) {
	filesystems := []map[string]any{
		{"VOLUMN": "/dev/disk1s1", "FILESYSTEM": "apfs", "TOTAL": 476801, "FREE": 273437, "TYPE": "/"},
		{"VOLUMN": "/dev/disk1s4", "FILESYSTEM": "apfs", "TOTAL": 10240, "FREE": 9216, "TYPE": "/private/var/vm"},
	}
	partitionInfo := map[string]map[string]string{
		"disk1s1": {
			"Total Size":  "500.3 GB (500277792768 Bytes)",
			"Volume UUID": "ABCD-1234",
			"File System": "APFS",
			"Volume Name": "Macintosh HD",
		},
	}
	drives := buildMacDrives(filesystems, partitionInfo, true)
	if len(drives) != 2 {
		t.Fatalf("got %d drives, want 2", len(drives))
	}
	// disk1s1 (root): diskutil overrides + FileVault.
	root := drives[0]
	if root["SERIAL"] != "ABCD-1234" || root["FILESYSTEM"] != "APFS" || root["LABEL"] != "Macintosh HD" {
		t.Errorf("root overrides = %v", root)
	}
	if root["ENCRYPT_STATUS"] != "Yes" || root["ENCRYPT_NAME"] != "FileVault 2" || root["ENCRYPT_ALGO"] != "XTS_AES_128" {
		t.Errorf("root FileVault = %v", root)
	}
	// disk1s4: no diskutil info, no FileVault (not root).
	if _, ok := drives[1]["ENCRYPT_STATUS"]; ok {
		t.Errorf("non-root should not be encrypted")
	}
}
