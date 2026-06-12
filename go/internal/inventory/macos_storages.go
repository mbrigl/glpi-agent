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

// buildMacDiscBurningStorages maps SPDiscBurningDataType to STORAGES, mirroring
// MacOS/Storages.pm _getDiscBurningStorages.
func buildMacDiscBurningStorages(root any) []map[string]any {
	storages := extractMacStorages(root, "_items")
	var out []map[string]any
	for _, name := range sortedStorageNames(storages) {
		hash := storages[name]
		nm := plistStr(hash, "_name")
		iface := "ATAPI"
		if plistStr(hash, "interconnect") == "SERIAL-ATA" {
			iface = "SATA"
		}
		manufacturer := getCanonicalManufacturer(winFirstNonEmpty(plistStr(hash, "manufacturer"), nm))
		model := nm
		if manufacturer != "" && model != "" {
			model = trimWhitespace(macStripManufacturer(model, manufacturer))
		}
		storage := map[string]any{"TYPE": "Disk burning", "INTERFACE": iface}
		setIf(storage, "NAME", winFirstNonEmpty(plistStr(hash, "bsd_name"), nm))
		setIf(storage, "MANUFACTURER", manufacturer)
		setIf(storage, "MODEL", model)
		setIf(storage, "FIRMWARE", trimWhitespace(plistStr(hash, "firmware")))
		if size := macDiskSizeMB(hash); size > 0 {
			storage["DISKSIZE"] = size
		}
		out = append(out, storage)
	}
	return out
}

// buildMacCardReaderStorages maps SPCardReaderDataType to STORAGES, mirroring
// MacOS/Storages.pm _getCardReaderStorages: the reader itself plus any inserted
// SD card.
func buildMacCardReaderStorages(root any) []map[string]any {
	storages := extractMacStorages(root, "_items")
	var out []map[string]any
	for _, name := range sortedStorageNames(storages) {
		hash := storages[name]
		if macStorageIsVolume(hash) {
			continue
		}
		nm := plistStr(hash, "_name")
		var storage map[string]any
		if nm == "spcardreader" {
			storage = map[string]any{"TYPE": "Card reader"}
			setIf(storage, "NAME", winFirstNonEmpty(plistStr(hash, "bsd_name"), nm))
			setIf(storage, "DESCRIPTION", nm)
			setIf(storage, "SERIAL", trimWhitespace(plistStr(hash, "spcardreader_serialnumber")))
			setIf(storage, "MODEL", nm)
			setIf(storage, "FIRMWARE", trimWhitespace(plistStr(hash, "spcardreader_revision-id")))
			setIf(storage, "MANUFACTURER", trimWhitespace(plistStr(hash, "spcardreader_vendor-id")))
		} else {
			storage = map[string]any{"TYPE": "SD Card"}
			setIf(storage, "NAME", winFirstNonEmpty(plistStr(hash, "bsd_name"), nm))
			setIf(storage, "DESCRIPTION", nm)
			if size := macDiskSizeMB(hash); size > 0 {
				storage["DISKSIZE"] = size
			}
		}
		out = append(out, storage)
	}
	return out
}

var macUSBStorageSkipRE = regexp.MustCompile(`(?i)keyboard|controller|IR Receiver|built-in|hub|mouse|tablet|usb(?:\d+)?bus`)

// buildMacUSBStorages maps the disk devices in SPUSBDataType (or FireWire) to
// STORAGES, mirroring MacOS/Storages.pm _getUSBStorages: the many non-disk USB
// device classes are filtered out. sublistkey is "_items" for USB, "units" for
// FireWire; iface is "USB" or "1394".
func buildMacUSBStorages(root any, sublistkey, iface string) []map[string]any {
	storages := extractMacStorages(root, sublistkey)
	var out []map[string]any
	for _, name := range sortedStorageNames(storages) {
		hash := storages[name]
		nm := plistStr(hash, "_name")
		// The upstream "bsn_name" guard is a typo that never matches, so the
		// filters always apply.
		if nm == "Mass Storage Device" || macUSBStorageSkipRE.MatchString(nm) {
			continue
		}
		if plistStr(hash, "Built-in_Device") == "Yes" {
			continue
		}
		if macStorageIsVolume(hash) {
			continue
		}

		storage := map[string]any{"TYPE": "Disk drive", "INTERFACE": iface}
		setIf(storage, "NAME", winFirstNonEmpty(plistStr(hash, "bsd_name"), nm))
		setIf(storage, "DESCRIPTION", nm)
		if size := macDiskSizeMB(hash); size > 0 {
			storage["DISKSIZE"] = size
		}
		extract := macInfoExtract(hash)
		setIf(storage, "MODEL", winFirstNonEmpty(extract["device_model"], nm))
		setIf(storage, "SERIAL", extract["serial_num"])
		setIf(storage, "FIRMWARE", extract["bcd_device"])
		if extract["manufacturer"] != "" {
			setIf(storage, "MANUFACTURER", getCanonicalManufacturer(extract["manufacturer"]))
		}
		out = append(out, storage)
	}
	return out
}

// macStorageIsVolume reports whether a storage node is a mounted volume rather
// than a device (skipped unless it carries a partition map).
func macStorageIsVolume(hash map[string]any) bool {
	hasContent := plistStr(hash, "iocontent") != "" || plistStr(hash, "file_system") != "" ||
		plistStr(hash, "mount_point") != ""
	return hasContent && plistStr(hash, "partition_map_type") == ""
}

// macInfoExtract pulls the serial_num/device_model/bcd_device/manufacturer/
// product_id fields (possibly prefixed) from a USB/FireWire device, mirroring
// MacOS/Storages.pm _getInfoExtract.
func macInfoExtract(hash map[string]any) map[string]string {
	re := regexp.MustCompile(`^(?:\w_)?(serial_num|device_model|bcd_device|manufacturer|product_id)`)
	out := map[string]string{}
	for k, v := range hash {
		if m := re.FindStringSubmatch(k); m != nil {
			if s, ok := v.(string); ok {
				out[m[1]] = strings.TrimSpace(s)
			}
		}
	}
	return out
}

// sortedStorageNames returns the storage map keys sorted.
func sortedStorageNames(storages map[string]map[string]any) []string {
	names := make([]string, 0, len(storages))
	for name := range storages {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
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
