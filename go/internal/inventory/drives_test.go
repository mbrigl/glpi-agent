// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestParseMountsAndBuildDrives(t *testing.T) {
	const mounts = `proc /proc proc rw,nosuid 0 0
/dev/nvme0n1p5 /workspace ext4 rw,relatime 0 0
tmpfs /dev/shm tmpfs rw,nosuid 0 0
overlay / overlay rw,relatime 0 0
/dev/sda1 /mnt/with\040space ext4 rw 0 0
`
	parsed := ParseMounts(strings.NewReader(mounts))
	if len(parsed) != 5 {
		t.Fatalf("ParseMounts got %d, want 5", len(parsed))
	}
	// Octal escape decoded.
	if parsed[4].Mountpoint != "/mnt/with space" {
		t.Errorf("mountpoint = %q, want '/mnt/with space'", parsed[4].Mountpoint)
	}

	statfs := func(mp string) (int, int, bool) { return 1000, 400, true }
	drives := BuildDrives(parsed, statfs)

	// proc, tmpfs and the overlay device are filtered out -> 2 real drives.
	if len(drives) != 2 {
		t.Fatalf("BuildDrives got %d, want 2 (pseudo/overlay filtered)", len(drives))
	}
	d := drives[0]
	if d["VOLUMN"] != "/dev/nvme0n1p5" || d["FILESYSTEM"] != "ext4" || d["TYPE"] != "/workspace" {
		t.Errorf("drive fields wrong: %v", d)
	}
	if d["TOTAL"] != 1000 || d["FREE"] != 400 {
		t.Errorf("drive size wrong: %v", d)
	}
}
