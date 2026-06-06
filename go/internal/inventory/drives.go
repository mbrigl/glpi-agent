// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"strings"
)

// Mount is one mounted filesystem from /proc/mounts.
type Mount struct {
	Device     string
	Mountpoint string
	FSType     string
}

// pseudoFilesystems are skipped from DRIVES, covering the upstream Linux/Drives.pm
// exclusion list plus the kernel pseudo-filesystems that /proc/mounts lists but
// `df` would not show.
var pseudoFilesystems = map[string]bool{
	"tmpfs": true, "devtmpfs": true, "usbfs": true, "proc": true, "devpts": true,
	"devshm": true, "udev": true, "sysfs": true, "cgroup": true, "cgroup2": true,
	"mqueue": true, "debugfs": true, "tracefs": true, "securityfs": true,
	"pstore": true, "bpf": true, "configfs": true, "fusectl": true,
	"hugetlbfs": true, "binfmt_misc": true, "autofs": true, "efivarfs": true,
	"ramfs": true, "nsfs": true,
}

// ParseMounts parses /proc/mounts into the mounted filesystems.
func ParseMounts(r io.Reader) []Mount {
	var mounts []Mount
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 3 {
			continue
		}
		mounts = append(mounts, Mount{
			Device:     unescapeMount(fields[0]),
			Mountpoint: unescapeMount(fields[1]),
			FSType:     fields[2],
		})
	}
	return mounts
}

// StatfsFunc returns the total and free space (in MiB) of a mountpoint.
type StatfsFunc func(mountpoint string) (totalMB, freeMB int, ok bool)

// BuildDrives assembles the DRIVES section from real (non-pseudo) mounted
// filesystems, mirroring the VOLUMN/FILESYSTEM/TYPE/TOTAL/FREE fields of
// getFilesystemsFromDf and the Linux/Drives.pm pseudo-fs filtering.
func BuildDrives(mounts []Mount, statfs StatfsFunc) []map[string]any {
	var drives []map[string]any
	for _, m := range mounts {
		if pseudoFilesystems[m.FSType] || m.Device == "overlay" {
			continue
		}
		drive := map[string]any{
			"VOLUMN":     m.Device,
			"TYPE":       m.Mountpoint,
			"FILESYSTEM": m.FSType,
		}
		if total, free, ok := statfs(m.Mountpoint); ok {
			drive["TOTAL"] = total
			drive["FREE"] = free
		}
		drives = append(drives, drive)
	}
	return drives
}

// unescapeMount decodes the octal escapes (\040 space, \011 tab, …) the kernel
// uses in /proc/mounts.
func unescapeMount(s string) string {
	if !strings.Contains(s, `\`) {
		return s
	}
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] == '\\' && i+3 < len(s) {
			if c, ok := octalByte(s[i+1 : i+4]); ok {
				b.WriteByte(c)
				i += 3
				continue
			}
		}
		b.WriteByte(s[i])
	}
	return b.String()
}

func octalByte(s string) (byte, bool) {
	var v int
	for _, c := range s {
		if c < '0' || c > '7' {
			return 0, false
		}
		v = v*8 + int(c-'0')
	}
	return byte(v), true
}
