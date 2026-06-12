// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// parseMacMountTypes returns the distinct filesystem types from `mount` output,
// mirroring Tools/Unix.pm getFilesystemsTypesFromMount (BSD "(ufs, ...)" form).
func parseMacMountTypes(output string) []string {
	bsd := regexp.MustCompile(`^\S+ on \S+ \((\w+)`)
	linux := regexp.MustCompile(`^\S+ on \S+ type (\w+)`)
	seen := map[string]bool{}
	var types []string
	for _, line := range strings.Split(output, "\n") {
		var t string
		if m := bsd.FindStringSubmatch(line); m != nil {
			t = m[1]
		} else if m := linux.FindStringSubmatch(line); m != nil {
			t = m[1]
		}
		if t != "" && !seen[t] {
			seen[t] = true
			types = append(types, t)
		}
	}
	return types
}

// parseMacDf parses `df -P -k -t <type>` output (no Type column) into filesystem
// entries, mirroring Tools/Unix.pm getFilesystemsFromDf: VOLUMN/FILESYSTEM/TOTAL/
// FREE/TYPE, sizes converted from KiB to MiB, virtual/zero filesystems skipped.
func parseMacDf(output, fsType string) []map[string]any {
	lines := strings.Split(strings.TrimRight(output, "\n"), "\n")
	if len(lines) < 2 {
		return nil
	}
	var filesystems []map[string]any
	for _, line := range lines[1:] {
		f := strings.Fields(line)
		if len(f) < 6 {
			continue
		}
		total, errT := strconv.Atoi(f[1])
		free, errF := strconv.Atoi(f[3])
		if errT != nil || errF != nil || total == 0 || free == 0 {
			continue
		}
		filesystems = append(filesystems, map[string]any{
			"VOLUMN":     f[0],
			"FILESYSTEM": fsType,
			"TOTAL":      total / 1024,
			"FREE":       free / 1024,
			"TYPE":       f[5],
		})
	}
	return filesystems
}

var macDiskutilPartRE = regexp.MustCompile(`(disk\d+s\d+)$`)

// parseMacDiskutilPartitions extracts the partition identifiers (diskNsM) from
// `diskutil list` output, mirroring MacOS/Drives.pm _getPartitions.
func parseMacDiskutilPartitions(output string) []string {
	var parts []string
	for _, line := range strings.Split(output, "\n") {
		if m := macDiskutilPartRE.FindStringSubmatch(line); m != nil {
			parts = append(parts, m[1])
		}
	}
	return parts
}

var macDiskutilInfoRE = regexp.MustCompile(`^\s*(\S[^:]+):\s+(\S.*\S)`)

// parseMacDiskutilInfo parses `diskutil info <partition>` "key: value" output,
// mirroring MacOS/Drives.pm _getPartitionInfo.
func parseMacDiskutilInfo(output string) map[string]string {
	info := map[string]string{}
	for _, line := range strings.Split(output, "\n") {
		if m := macDiskutilInfoRE.FindStringSubmatch(line); m != nil {
			info[strings.TrimSpace(m[1])] = m[2]
		}
	}
	return info
}

var macTotalSizeRE = regexp.MustCompile(`^([.\d]+\s\S+)`)

// buildMacDrives combines the df filesystems with the per-partition diskutil info
// and the FileVault status into the DRIVES section, mirroring MacOS/Drives.pm
// doInventory: diskutil overrides TOTAL/SERIAL/FILESYSTEM/LABEL, and the root
// filesystem gets the FileVault 2 encryption fields when enabled. Only df
// filesystems matched to a partition are emitted, sorted by VOLUMN.
func buildMacDrives(filesystems []map[string]any, partitionInfo map[string]map[string]string, fileVaultOn bool) []map[string]any {
	byVolume := map[string]map[string]any{}
	for _, fs := range filesystems {
		byVolume[fs["VOLUMN"].(string)] = fs
	}

	for partition, info := range partitionInfo {
		fs := byVolume["/dev/"+partition]
		if fs == nil {
			continue
		}
		if m := macTotalSizeRE.FindStringSubmatch(info["Total Size"]); m != nil {
			if total := canonicalSizeMB(m[1]); total > 0 {
				fs["TOTAL"] = total
			}
		}
		setIf(fs, "SERIAL", winFirstNonEmpty(info["Volume UUID"], info["UUID"]))
		setIf(fs, "FILESYSTEM", winFirstNonEmpty(info["File System"], info["Partition Type"]))
		setIf(fs, "LABEL", info["Volume Name"])
	}

	if fileVaultOn {
		for _, fs := range byVolume {
			if fs["TYPE"] == "/" {
				fs["ENCRYPT_STATUS"] = "Yes"
				fs["ENCRYPT_NAME"] = "FileVault 2"
				fs["ENCRYPT_ALGO"] = "XTS_AES_128"
			}
		}
	}

	volumes := make([]string, 0, len(byVolume))
	for v := range byVolume {
		volumes = append(volumes, v)
	}
	sort.Strings(volumes)
	var drives []map[string]any
	for _, v := range volumes {
		drives = append(drives, byVolume[v])
	}
	return drives
}
