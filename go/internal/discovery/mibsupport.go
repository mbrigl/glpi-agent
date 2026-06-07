// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"regexp"
	"sort"
	"strings"
)

// sysORID is the OID of the sysORTable mib-support column walked to match the
// `oid`-rule MibSupport modules.
const sysORID = "1.3.6.1.2.1.1.9.1.2"

// MibModule is a Go port of one upstream SNMP/MibSupport/* module: a set of
// match rules plus the value accessors GLPI::Agent::SNMP::Device calls. Each
// accessor reads from the device over SNMP and returns "" when not applicable.
//
// Matching mirrors MibSupport.pm: a module applies when its SysObjectID regex
// matches the device sysObjectID, or when its OID rule is present in the
// device's sysORID table. Lower Priority wins first.
type MibModule struct {
	Name        string
	SysObjectID *regexp.Regexp
	OID         string // matched against the sysORID table
	PrivateOID  string // matched when a GET on it responds
	Priority    int

	Type         func(g SNMPGetter, d Device) string
	Manufacturer func(g SNMPGetter, d Device) string
	Model        func(g SNMPGetter, d Device) string
	Serial       func(g SNMPGetter, d Device) string
	Firmware     func(g SNMPGetter, d Device) string
	Mac          func(g SNMPGetter, d Device) string
	SnmpHostname func(g SNMPGetter, d Device) string
}

// mibRegistry holds every registered vendor module.
var mibRegistry []MibModule

// registerMib adds a module to the registry (called from each vendor file's
// init).
func registerMib(m MibModule) {
	if m.Priority == 0 {
		m.Priority = 10 // MibSupportTemplate default
	}
	mibRegistry = append(mibRegistry, m)
}

// oidMatch builds the prefix-match regex an upstream module gets from
// getRegexpOidMatch(oid): the dotted OID anchored at the start.
func oidMatch(oid string) *regexp.Regexp {
	escaped := strings.ReplaceAll(strings.TrimPrefix(oid, "."), ".", `\.`)
	return regexp.MustCompile(`^\.?` + escaped)
}

// matchMibModules returns the modules that apply to a device, sorted by
// priority. sysorid is the set of OIDs advertised in the device's sysORID table.
func matchMibModules(sysObjectID string, sysorid map[string]bool, getter SNMPGetter) []MibModule {
	var matched []MibModule
	for _, m := range mibRegistry {
		switch {
		case m.SysObjectID != nil && sysObjectID != "" && m.SysObjectID.MatchString(sysObjectID):
			matched = append(matched, m)
		case m.OID != "" && sysorid[strings.TrimPrefix(m.OID, ".")]:
			matched = append(matched, m)
		case m.PrivateOID != "" && getter != nil && getOne(getter, m.PrivateOID) != "":
			matched = append(matched, m)
		}
	}
	sort.SliceStable(matched, func(i, j int) bool { return matched[i].Priority < matched[j].Priority })
	return matched
}

// applyMibSupport refines a device's identity fields using the matching vendor
// modules, mirroring the getXByMibSupport precedence in SNMP/Device.pm. The
// first matching module to provide a field wins, overriding the generic
// sysObjectID-database classification.
func applyMibSupport(device Device, getter SNMPGetter, sysObjectID string) {
	modules := matchMibModules(sysObjectID, walkSysORIDSet(getter), getter)
	if len(modules) == 0 {
		return
	}

	fields := []struct {
		key  string
		pick func(MibModule) func(SNMPGetter, Device) string
	}{
		{"TYPE", func(m MibModule) func(SNMPGetter, Device) string { return m.Type }},
		{"MANUFACTURER", func(m MibModule) func(SNMPGetter, Device) string { return m.Manufacturer }},
		{"MODEL", func(m MibModule) func(SNMPGetter, Device) string { return m.Model }},
		{"SERIAL", func(m MibModule) func(SNMPGetter, Device) string { return m.Serial }},
		{"FIRMWARE", func(m MibModule) func(SNMPGetter, Device) string { return m.Firmware }},
		{"MAC", func(m MibModule) func(SNMPGetter, Device) string { return m.Mac }},
		{"SNMPHOSTNAME", func(m MibModule) func(SNMPGetter, Device) string { return m.SnmpHostname }},
	}
	// First matching module to yield a value wins per field (priority order),
	// overriding the generic classification.
	for _, field := range fields {
		for _, m := range modules {
			fn := field.pick(m)
			if fn == nil {
				continue
			}
			if v := strings.TrimSpace(fn(getter, device)); v != "" {
				device[field.key] = v
				break
			}
		}
	}
}

// walkSysORIDSet walks the sysORID table and returns the set of advertised mib
// OIDs (trimmed of any leading dot).
func walkSysORIDSet(getter SNMPGetter) map[string]bool {
	set := map[string]bool{}
	walked, err := getter.Walk(sysORID)
	if err != nil {
		return set
	}
	for _, v := range walked {
		set[strings.TrimPrefix(strings.TrimSpace(v), ".")] = true
	}
	return set
}
