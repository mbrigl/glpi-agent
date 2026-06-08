// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"sort"
	"strconv"
	"strings"
)

// ENTITY-MIB entPhysicalEntry and the per-field column suffixes
// (SNMP/Device/Components.pm %physical_components_variables).
const (
	oidEntPhysicalEntry = "1.3.6.1.2.1.47.1.1.1.1"
	// Dell-Vendor-MIB private chassis serial table.
	oidDellProductSerial = "1.3.6.1.4.1.674.10895.3000.1.2.100.8.1.2"
	// Cisco physical-entity MAC/IP extension tables.
	oidCiscoEntityMacIndex = "1.3.6.1.4.1.9.9.513.1.1.1.1.4"
	oidCiscoEntityMac      = "1.3.6.1.4.1.9.9.513.1.1.1.1.2"
	oidCiscoEntityIP       = "1.3.6.1.4.1.14179.2.2.1.1.19"
)

// componentColumn describes one entPhysical column: its suffix under
// entPhysicalEntry and how the raw value is normalised.
type componentColumn struct {
	field  string
	suffix string
	kind   string // "constant" | "string" | "type" | "mac"
}

// componentColumns mirror %physical_components_variables (INDEX handled
// specially). Order does not matter; they are applied per index.
var componentColumns = []componentColumn{
	{"NAME", "7", "string"},
	{"DESCRIPTION", "2", "string"},
	{"SERIAL", "11", "string"},
	{"MODEL", "13", "string"},
	{"TYPE", "5", "type"},
	{"FRU", "16", "constant"},
	{"MANUFACTURER", "12", "string"},
	{"FIRMWARE", "9", "string"},
	{"REVISION", "8", "string"},
	{"VERSION", "10", "string"},
	{"CONTAINEDININDEX", "4", "constant"},
}

// entPhysicalClassTypes maps entPhysicalClass values to the TYPE label
// (numeric form, as gosnmp renders the enum).
var entPhysicalClassTypes = map[string]string{
	"1": "other", "2": "unknown", "3": "chassis", "4": "backplane",
	"5": "container", "6": "powerSupply", "7": "fan", "8": "sensor",
	"9": "module", "10": "port", "11": "stack", "12": "cpu",
}

// BuildPhysicalComponents walks the ENTITY-MIB physical-entity table and returns
// the device COMPONENTS, mirroring SNMP/Device/Components.pm. It returns nil when
// the device exposes no entPhysical table.
func BuildPhysicalComponents(getter SNMPGetter) []map[string]any {
	walk, err := getter.Walk(oidEntPhysicalEntry)
	if err != nil || len(walk) == 0 {
		return nil
	}

	// Split each "<column>.<index>" leaf into per-field index->value tables.
	// INDEX is column 1.
	walks := map[string]map[string]string{}
	indexCol := map[string]string{}
	suffixToField := map[string]string{"1": "INDEX"}
	for _, c := range componentColumns {
		suffixToField[c.suffix] = c.field
	}
	for leaf, val := range walk {
		col, idx, ok := strings.Cut(leaf, ".")
		if !ok {
			continue
		}
		field, ok := suffixToField[col]
		if !ok {
			continue
		}
		if field == "INDEX" {
			indexCol[idx] = val
			continue
		}
		if walks[field] == nil {
			walks[field] = map[string]string{}
		}
		walks[field][idx] = val
	}

	// Determine the index set: the INDEX column when present, else the keys of
	// the most-populated column.
	var indexes []string
	if len(indexCol) > 0 {
		for _, v := range indexCol {
			indexes = append(indexes, strings.TrimSpace(v))
		}
	} else {
		var best map[string]string
		for _, tbl := range walks {
			if len(tbl) > len(best) {
				best = tbl
			}
		}
		for k := range best {
			indexes = append(indexes, k)
		}
	}
	if len(indexes) == 0 {
		return nil
	}
	sort.Slice(indexes, func(i, j int) bool { return ifIndexLess(indexes[i], indexes[j]) })

	// Cisco devices expose MAC/IP via a separate index table.
	if macIndex, _ := getter.Walk(oidCiscoEntityMacIndex); len(macIndex) > 0 {
		macs, _ := getter.Walk(oidCiscoEntityMac)
		ips, _ := getter.Walk(oidCiscoEntityIP)
		for suffix, index := range macIndex {
			if mac := macs[suffix]; mac != "" {
				if walks["MAC"] == nil {
					walks["MAC"] = map[string]string{}
				}
				walks["MAC"][index] = mac
			}
			if ip := ips[suffix]; ip != "" {
				if walks["IP"] == nil {
					walks["IP"] = map[string]string{}
				}
				walks["IP"][index] = ip
			}
		}
	}

	// Dell chassis serials come from a private OID when more than one is present.
	dellSN, _ := getter.Walk(oidDellProductSerial)
	if len(dellSN) <= 1 {
		dellSN = nil
	}

	components := make([]map[string]any, 0, len(indexes))
	module := 0
	for _, index := range indexes {
		comp := map[string]any{"INDEX": componentIndexValue(indexCol[index], index)}
		for _, c := range componentColumns {
			raw, ok := walks[c.field][index]
			if !ok {
				continue
			}
			if v := normalizeComponentValue(c.kind, raw); v != "" {
				comp[c.field] = v
			}
		}
		if mac := walks["MAC"][index]; mac != "" {
			comp["MAC"] = strings.TrimSpace(mac)
		}
		if ip := walks["IP"][index]; ip != "" {
			comp["IP"] = strings.TrimSpace(ip)
		}

		// Fix chassis serials for Dell devices from the private serial table.
		if dellSN != nil {
			if t, _ := comp["TYPE"].(string); t == "chassis" {
				if name, _ := comp["NAME"].(string); strings.HasPrefix(name, "Unit ") {
					if n, err := strconv.Atoi(strings.TrimSpace(strings.TrimPrefix(name, "Unit "))); err == nil {
						module = n
					}
				} else {
					module++
				}
				if serial := strings.TrimSpace(dellSN[strconv.Itoa(module)]); serial != "" {
					comp["SERIAL"] = serial
				}
			}
		}

		components = append(components, comp)
	}
	return components
}

// componentIndexValue returns the entPhysicalIndex value, falling back to the
// row suffix (getCanonicalConstant($walks{INDEX}->{$index} || $index)).
func componentIndexValue(indexVal, fallback string) string {
	if v := strings.TrimSpace(indexVal); v != "" {
		return v
	}
	return strings.TrimSpace(fallback)
}

// normalizeComponentValue applies the per-column normalisation of the upstream
// getPhysicalComponents dispatch.
func normalizeComponentValue(kind, raw string) string {
	switch kind {
	case "type":
		return entPhysicalClassTypes[strings.TrimSpace(raw)]
	case "mac":
		return canonicalMAC(raw)
	default: // constant / string
		return strings.TrimSpace(raw)
	}
}
