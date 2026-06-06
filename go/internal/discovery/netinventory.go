// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"sort"
	"strconv"
	"strings"
)

// IF-MIB column OIDs used to build the PORTS table (SNMP/Hardware.pm).
const (
	oidIfDescr       = "1.3.6.1.2.1.2.2.1.2"
	oidIfType        = "1.3.6.1.2.1.2.2.1.3"
	oidIfSpeed       = "1.3.6.1.2.1.2.2.1.5"
	oidIfPhysAddress = "1.3.6.1.2.1.2.2.1.6"
	oidIfName        = "1.3.6.1.2.1.31.1.1.1.1"
)

// GetInventory builds a full SNMP inventory of one device: the generic
// properties (as in NetDiscovery) plus the IF-MIB PORTS table. Mirrors the
// device assembly of GLPI::Agent::Task::NetInventory + SNMP/Hardware.pm.
//
// The sysObjectID-driven TYPE/MANUFACTURER/MODEL classification and the vendor
// MibSupport-specific sections (CARTRIDGES, PAGECOUNTERS, LLDP/CDP connections,
// VLANs, …) are the follow-on MIB tail.
func GetInventory(ip string, getter SNMPGetter) (Device, error) {
	values, err := getter.Get(genericOIDs)
	if err != nil {
		return nil, err
	}
	device := BuildDevice(ip, values)
	if device == nil {
		return nil, nil
	}

	ports, err := buildPorts(getter)
	if err != nil {
		return nil, err
	}
	if len(ports) > 0 {
		device["PORTS"] = ports
	}
	return device, nil
}

// buildPorts walks the IF-MIB interface columns and assembles one PORT entry per
// interface index, keyed by the canonical PORT field names.
func buildPorts(getter SNMPGetter) ([]map[string]any, error) {
	columns := map[string]string{
		"IFDESCR": oidIfDescr,
		"IFTYPE":  oidIfType,
		"IFSPEED": oidIfSpeed,
		"MAC":     oidIfPhysAddress,
		"IFNAME":  oidIfName,
	}

	// index -> field -> value
	byIndex := map[string]map[string]any{}
	for field, oid := range columns {
		walked, err := getter.Walk(oid)
		if err != nil {
			return nil, err
		}
		for idx, val := range walked {
			val = strings.TrimSpace(val)
			if val == "" {
				continue
			}
			if byIndex[idx] == nil {
				byIndex[idx] = map[string]any{}
			}
			byIndex[idx][field] = val
		}
	}

	ports := make([]map[string]any, 0, len(byIndex))
	for idx, port := range byIndex {
		port["IFNUMBER"] = idx
		// ifName falls back to ifDescr (SNMP/Hardware.pm IFNAME definition).
		if _, ok := port["IFNAME"]; !ok {
			if descr, ok := port["IFDESCR"]; ok {
				port["IFNAME"] = descr
			}
		}
		ports = append(ports, port)
	}

	// Sort by interface number, as the Perl output is sorted by ifIndex.
	sort.Slice(ports, func(i, j int) bool {
		return ifIndexLess(ports[i]["IFNUMBER"].(string), ports[j]["IFNUMBER"].(string))
	})
	return ports, nil
}

func ifIndexLess(a, b string) bool {
	ai, aerr := strconv.Atoi(a)
	bi, berr := strconv.Atoi(b)
	if aerr == nil && berr == nil {
		return ai < bi
	}
	return a < b
}
