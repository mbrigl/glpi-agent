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
// properties (as in NetDiscovery), the sysObjectID-driven
// TYPE/MANUFACTURER/MODEL classification, the device SERIAL/FIRMWARE/MAC, and
// the IF-MIB PORTS table. Mirrors the device assembly of
// GLPI::Agent::Task::NetInventory + SNMP/Device.pm + SNMP/Hardware.pm.
//
// The per-vendor MibSupport refinements (CARTRIDGES, PAGECOUNTERS, LLDP/CDP
// connections, VLANs, …) are the follow-on MIB tail.
func GetInventory(ip string, getter SNMPGetter) (Device, error) {
	values, err := getter.Get(genericOIDs)
	if err != nil {
		return nil, err
	}
	device := BuildDevice(ip, values)
	if device == nil {
		return nil, nil
	}

	// Match the vendor MibSupport modules once and reuse them for the identity
	// overrides, the components and the run hooks (MibSupport->new is built once
	// per device in SNMP/Hardware.pm).
	sysoid, _ := device["SYSOBJECTID"].(string)
	var modules []MibModule
	if sysoid != "" {
		modules = matchMibModules(sysoid, walkSysORIDSet(getter), getter)
	}

	setIdentity(getter, device, modules)

	ports, err := buildPorts(getter)
	if err != nil {
		return nil, err
	}
	if len(ports) > 0 {
		device["PORTS"] = ports
	}
	// MAC falls back to a unique interface address when no bridge address is set
	// (SNMP/Device.pm setMacAddress fallback).
	if _, ok := device["MAC"]; !ok {
		if mac := uniquePortMAC(ports); mac != "" {
			device["MAC"] = mac
		}
	}

	// setComponents() then runMibSupport(), in that order (SNMP/Hardware.pm), so
	// a run hook can fix up components produced by a Components accessor.
	setComponentsByMib(device, getter, modules)
	runMibSupport(device, getter, modules)
	return device, nil
}

// Device-level OIDs from SNMP/Device.pm (setSerial/setFirmware/setMacAddress/
// setModel).
const (
	oidEntPhysicalSerialNum   = "1.3.6.1.2.1.47.1.1.1.1.11"
	oidPrtGeneralSerialNum    = "1.3.6.1.2.1.43.5.1.1.17"
	oidEntPhysicalSoftwareRev = "1.3.6.1.2.1.47.1.1.1.1.10"
	oidEntPhysicalFirmwareRev = "1.3.6.1.2.1.47.1.1.1.1.9"
	oidEntPhysicalModelName   = "1.3.6.1.2.1.47.1.1.1.1.13"
	oidDot1dBaseBridgeAddr    = "1.3.6.1.2.1.17.1.1.0"
	oidPrinterModel           = "1.3.6.1.2.1.25.3.2.1.3.1"
	oidUPSModel               = "1.3.6.1.2.1.33.1.1.5.0"
)

// vendorSerialOIDs / vendorFirmwareOIDs mirror the vendor-specific fallback lists
// in setSerial/setFirmware.
var vendorSerialOIDs = []string{
	"1.3.6.1.4.1.2636.3.1.3.0", "1.3.6.1.4.1.248.14.1.1.9.1.10.1",
	"1.3.6.1.4.1.253.8.53.3.2.1.3.1", "1.3.6.1.4.1.367.3.2.1.2.1.4.0",
	"1.3.6.1.4.1.1602.1.2.1.4.0", "1.3.6.1.4.1.2435.2.3.9.4.2.1.5.5.1.0",
	"1.3.6.1.4.1.318.1.1.4.1.5.0", "1.3.6.1.4.1.6027.3.8.1.1.5.0",
	"1.3.6.1.4.1.6027.3.10.1.2.2.1.12.1", "1.3.6.1.4.1.3417.2.11.1.4.0",
	"1.3.6.1.4.1.232.2.2.2.1.0", "1.3.6.1.4.1.232.11.2.10.3.0",
}
var vendorFirmwareOIDs = []string{
	"1.3.6.1.4.1.9.9.25.1.1.1.2.5", "1.3.6.1.4.1.248.14.1.1.2.0",
	"1.3.6.1.4.1.2636.3.40.1.4.1.1.1.5.0",
}

// setIdentity fills TYPE/MANUFACTURER/MODEL (from the sysObjectID database) and
// the device SERIAL/FIRMWARE/MAC, mirroring the generic (non-MibSupport) path of
// SNMP/Device.pm. The per-vendor MibSupport refinements are follow-on.
func setIdentity(getter SNMPGetter, device Device, modules []MibModule) {
	if sysoid, ok := device["SYSOBJECTID"].(string); ok && sysoid != "" {
		if c, ok := classifyBySysObjectID(sysoid); ok {
			if c.Type != "" {
				device["TYPE"] = c.Type
			}
			if c.Manufacturer != "" {
				device["MANUFACTURER"] = c.Manufacturer
			}
			if c.Model != "" {
				device["MODEL"] = c.Model
			}
		}
	}

	// SERIAL: entPhysicalSerialNum, prtGeneralSerialNumber, then vendor OIDs.
	serial := firstNonEmpty(
		walkFirst(getter, oidEntPhysicalSerialNum),
		walkFirst(getter, oidPrtGeneralSerialNum),
		getFirstOID(getter, vendorSerialOIDs),
	)
	if serial = strings.TrimSpace(serial); serial != "" && !isAllX(serial) {
		device["SERIAL"] = serial
	}

	// FIRMWARE: entPhysicalSoftwareRev, entPhysicalFirmwareRev, then vendor OIDs.
	firmware := firstNonEmpty(
		walkFirst(getter, oidEntPhysicalSoftwareRev),
		walkFirst(getter, oidEntPhysicalFirmwareRev),
		getFirstOID(getter, vendorFirmwareOIDs),
	)
	if firmware = strings.TrimSpace(firmware); firmware != "" {
		device["FIRMWARE"] = firmware
	}

	// MAC: dot1dBaseBridgeAddress (the interface fallback is applied by the caller).
	if mac := strings.TrimSpace(getOne(getter, oidDot1dBaseBridgeAddr)); mac != "" {
		device["MAC"] = mac
	}

	// Vendor MibSupport refinements override the generic classification (the
	// getXByMibSupport precedence in SNMP/Device.pm).
	applyMibSupportFields(device, getter, modules)

	// MODEL fallback when the database had none.
	if _, ok := device["MODEL"]; !ok {
		var model string
		switch device["TYPE"] {
		case "PRINTER":
			model = getOne(getter, oidPrinterModel)
		case "POWER":
			model = getOne(getter, oidUPSModel)
		default:
			model = walkFirst(getter, oidEntPhysicalModelName)
		}
		if model = strings.TrimSpace(model); model != "" {
			device["MODEL"] = model
		}
	}
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

// getOne does a single SNMP GET and returns the trimmed value (SNMP/Device.pm get).
func getOne(getter SNMPGetter, oid string) string {
	oid = strings.TrimPrefix(oid, ".")
	m, err := getter.Get([]string{oid})
	if err != nil {
		return ""
	}
	return strings.TrimSpace(m[oid])
}

// walkFirst walks a table column and returns the value at the lowest index,
// mirroring SNMP/Device.pm get_first.
func walkFirst(getter SNMPGetter, base string) string {
	base = strings.TrimPrefix(base, ".")
	m, err := getter.Walk(base)
	if err != nil || len(m) == 0 {
		return ""
	}
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Slice(keys, func(i, j int) bool { return oidSuffixLess(keys[i], keys[j]) })
	for _, k := range keys {
		if v := strings.TrimSpace(m[k]); v != "" {
			return v
		}
	}
	return ""
}

// getFirstOID returns the first non-empty value across single GETs of oids.
func getFirstOID(getter SNMPGetter, oids []string) string {
	for _, oid := range oids {
		if v := getOne(getter, oid); v != "" {
			return v
		}
	}
	return ""
}

// uniquePortMAC returns the single distinct port MAC, or "" if there are none or
// several (SNMP/Device.pm setMacAddress interface fallback, simplified).
func uniquePortMAC(ports []map[string]any) string {
	seen := map[string]bool{}
	for _, p := range ports {
		if mac, ok := p["MAC"].(string); ok && mac != "" {
			seen[mac] = true
		}
	}
	if len(seen) == 1 {
		for mac := range seen {
			return mac
		}
	}
	return ""
}

// oidSuffixLess orders OID index suffixes by their numeric components.
func oidSuffixLess(a, b string) bool {
	as, bs := strings.Split(a, "."), strings.Split(b, ".")
	for i := 0; i < len(as) && i < len(bs); i++ {
		ai, aerr := strconv.Atoi(as[i])
		bi, berr := strconv.Atoi(bs[i])
		if aerr == nil && berr == nil {
			if ai != bi {
				return ai < bi
			}
			continue
		}
		if as[i] != bs[i] {
			return as[i] < bs[i]
		}
	}
	return len(as) < len(bs)
}

// firstNonEmpty returns the first non-empty string.
func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}

// isAllX reports whether the serial is a well-known placeholder of only X's
// (SNMP/Device.pm setSerial skips these).
func isAllX(s string) bool {
	if s == "" {
		return false
	}
	for _, c := range s {
		if c != 'X' && c != 'x' {
			return false
		}
	}
	return true
}
