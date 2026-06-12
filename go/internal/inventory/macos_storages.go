// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// recSubStorage flattens the nested storage tree, collecting every dict that has
// an "_name", mirroring Tools/MacOS.pm _recSubStorage. sublistkey is the array
// key recursed into below the top level ("_items", or "units" for FireWire).
func recSubStorage(list []any, sublistkey string, depth int) []map[string]any {
	listkey := "_items"
	if depth > 0 && sublistkey != "" {
		listkey = sublistkey
	}
	var nodes []map[string]any
	for _, n := range list {
		node, ok := n.(map[string]any)
		if !ok {
			continue
		}
		if sub, ok := node[listkey].([]any); ok {
			nodes = append(nodes, recSubStorage(sub, sublistkey, depth+1)...)
		}
		if _, ok := node["_name"]; ok {
			nodes = append(nodes, node)
		}
	}
	return nodes
}

// extractMacStorages returns the storage dicts keyed by _name, mirroring
// Tools/MacOS.pm _extractStoragesFromXml.
func extractMacStorages(root any, sublistkey string) map[string]map[string]any {
	items := plistDictArray(root, "")
	storages := map[string]map[string]any{}
	for _, node := range recSubStorage(items, sublistkey, 0) {
		if name := plistStr(node, "_name"); name != "" {
			storages[name] = node
		}
	}
	return storages
}

// buildMacATAStorages maps SPSerialATADataType / SPNVMeDataType disks to the
// STORAGES section, mirroring MacOS/Storages.pm _getSerialATAStorages /
// _getNVMeStorages: a disk needs a partition map (SATA also accepts a detachable
// drive) and must not be a controller. iface is "SATA" or "NVME".
func buildMacATAStorages(root any, iface string, allowDetachable bool) []map[string]any {
	storages := extractMacStorages(root, "_items")
	names := make([]string, 0, len(storages))
	for name := range storages {
		names = append(names, name)
	}
	sort.Strings(names)

	var out []map[string]any
	for _, name := range names {
		hash := storages[name]
		hasMap := plistStr(hash, "partition_map_type") != "" ||
			(allowDetachable && plistStr(hash, "detachable_drive") != "")
		if !hasMap {
			continue
		}
		nm := plistStr(hash, "_name")
		if strings.Contains(strings.ToLower(nm), "controller") {
			continue
		}

		manufacturer := getCanonicalManufacturer(nm)
		model := winFirstNonEmpty(plistStr(hash, "device_model"), nm)
		if manufacturer != "" && model != "" {
			model = trimWhitespace(macStripManufacturer(model, manufacturer))
		}

		storage := map[string]any{
			"TYPE":      "Disk drive",
			"INTERFACE": iface,
		}
		setIf(storage, "NAME", winFirstNonEmpty(plistStr(hash, "bsd_name"), nm))
		setIf(storage, "MANUFACTURER", manufacturer)
		setIf(storage, "SERIAL", trimWhitespace(plistStr(hash, "device_serial")))
		setIf(storage, "MODEL", model)
		setIf(storage, "FIRMWARE", trimWhitespace(plistStr(hash, "device_revision")))
		setIf(storage, "DESCRIPTION", trimWhitespace(nm))
		if size := macDiskSizeMB(hash); size > 0 {
			storage["DISKSIZE"] = size
		}
		out = append(out, storage)
	}
	return out
}

// macStripManufacturer removes the first "\s*<manufacturer>\s*" run (case-
// insensitive) from a model string (MacOS/Storages.pm "Cleanup manufacturer from
// model").
func macStripManufacturer(model, manufacturer string) string {
	re := regexp.MustCompile(`(?i)\s*` + regexp.QuoteMeta(manufacturer) + `\s*`)
	if loc := re.FindStringIndex(model); loc != nil {
		return model[:loc[0]] + model[loc[1]:]
	}
	return model
}

// macDiskSizeMB returns a storage size in MiB, preferring size_in_bytes,
// mirroring MacOS/Storages.pm _setDiskSize (getCanonicalSize(..., 1024)).
func macDiskSizeMB(hash map[string]any) int {
	if b := plistStr(hash, "size_in_bytes"); b != "" {
		if n, err := strconv.ParseInt(b, 10, 64); err == nil {
			return int(n / 1024 / 1024)
		}
	}
	if size := plistStr(hash, "size"); size != "" {
		return canonicalSizeMB(size)
	}
	return 0
}

// plistStr returns a string value from a parsed-plist dict ("" when absent).
func plistStr(node map[string]any, key string) string {
	if node == nil {
		return ""
	}
	s, _ := node[key].(string)
	return strings.TrimSpace(s)
}
