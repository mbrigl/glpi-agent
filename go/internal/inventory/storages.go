// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// BuildStorages collects the STORAGES section from the block devices under
// <root>/sys/block, mirroring the sysfs path of Linux/Storages.pm + Tools/Linux.
// root is "" for the live system; tests point it at a fixture tree.
//
// Fields: NAME, TYPE (disk), DISKSIZE (MiB, from the 512-byte sector count),
// MODEL, MANUFACTURER (device/vendor, dropped when "ATA"), FIRMWARE (device/rev).
func BuildStorages(root string) []map[string]any {
	matches, _ := filepath.Glob(filepath.Join(root, "sys/block/*/device"))

	var storages []map[string]any
	for _, deviceDir := range matches {
		blockDir := filepath.Dir(deviceDir)
		name := filepath.Base(blockDir)

		storage := map[string]any{"NAME": name, "TYPE": "disk"}
		if size := readSysLine(filepath.Join(blockDir, "size")); size != "" {
			if sectors, err := strconv.ParseInt(size, 10, 64); err == nil && sectors > 0 {
				storage["DISKSIZE"] = int(sectors * 512 / (1024 * 1024))
			}
		}
		setIf(storage, "MODEL", readSysLine(filepath.Join(deviceDir, "model")))
		setIf(storage, "FIRMWARE", readSysLine(filepath.Join(deviceDir, "rev")))
		if vendor := readSysLine(filepath.Join(deviceDir, "vendor")); vendor != "" && vendor != "ATA" {
			storage["MANUFACTURER"] = vendor
		}
		storages = append(storages, storage)
	}
	return storages
}

// readSysLine reads the first line of a sysfs attribute, trimmed.
func readSysLine(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(strings.SplitN(string(data), "\n", 2)[0])
}
