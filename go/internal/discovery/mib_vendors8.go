// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"encoding/hex"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// Eighth batch of upstream SNMP/MibSupport/* vendor modules: the index- and
// conditional-logic ones (EMC, Force10S, Panasas, Siemens, FreeBSD/Stormshield),
// ported verbatim from the Perl OIDs. LinuxAppliance remains follow-on: it needs
// a manufacturer-ID database plus process / installed-software walks.

func init() {
	registerEMC()
	registerForce10S()
	registerPanasas()
	registerSiemens()
	registerFreeBSD()
}

// --- EMC (Fibre-Channel storage, FCMGMT-MIB) ---
func registerEMC() {
	const (
		emc             = "1.3.6.1.4.1.674"
		connUnitEntry   = "1.3.6.1.3.94.1.6.1"
		connUnitID      = connUnitEntry + ".1"
		connUnitProduct = connUnitEntry + ".7"
		connUnitSn      = connUnitEntry + ".8"
	)
	// lowestUnit returns the first connUnit index (sorted), or "" when the FC
	// table is absent — the guard that avoids resetting Dell printers.
	lowestUnit := func(g SNMPGetter) string {
		units, _ := g.Walk(connUnitID)
		keys := sortedWalkKeys(units)
		if len(keys) == 0 {
			return ""
		}
		return keys[0]
	}
	registerMib(MibModule{
		Name:        "emc",
		SysObjectID: oidMatch(emc),
		Type: func(g SNMPGetter, _ Device) string {
			if lowestUnit(g) == "" {
				return ""
			}
			return "NETWORKING"
		},
		Serial: func(g SNMPGetter, _ Device) string {
			unit := lowestUnit(g)
			if unit == "" {
				return ""
			}
			return mibGet(g, connUnitSn+"."+unit)
		},
		Model: func(g SNMPGetter, _ Device) string {
			unit := lowestUnit(g)
			if unit == "" {
				return ""
			}
			return mibGet(g, connUnitProduct+"."+unit)
		},
	})
}

// --- Force10 S-series (F10-S-SERIES-CHASSIS-MIB stack + ports) ---
func registerForce10S() {
	const (
		force10S         = "1.3.6.1.4.1.6027.1.3"
		chStackUnitEntry = "1.3.6.1.4.1.6027.3.10.1.2.2.1"
		chSysPortIfIndex = "1.3.6.1.4.1.6027.3.10.1.2.5.1.5"
	)
	// stack-unit columns (suffix under chStackUnitEntry).
	cols := []componentColumn{
		{"MODEL", "7", "string"},
		{"DESCRIPTION", "9", "string"},
		{"FIRMWARE", "10", "string"},
		{"SERIAL", "12", "string"},
		{"REVISION", "21", "string"},
	}
	registerMib(MibModule{
		Name:        "Force10 S-series",
		SysObjectID: oidMatch(force10S),
		Components: func(g SNMPGetter, _ Device) []map[string]any {
			components := force10StackUnits(g, chStackUnitEntry, cols)
			components = append(components, force10Ports(g, chSysPortIfIndex)...)
			if len(components) > 0 {
				components = append(components, map[string]any{
					"CONTAINEDININDEX": "0",
					"INDEX":            "-1",
					"TYPE":             "stack",
					"NAME":             "Force10 S-series Stack",
				})
			}
			return components
		},
	})
}

// force10StackUnits builds the chassis components from the chStackUnit table.
func force10StackUnits(g SNMPGetter, base string, cols []componentColumn) []map[string]any {
	walk, _ := g.Walk(base)
	if len(walk) == 0 {
		return nil
	}
	// INDEX is column 2 (chStackUnitNumber); split the rest per supported column.
	suffixToField := map[string]string{"2": "INDEX"}
	for _, c := range cols {
		suffixToField[c.suffix] = c.field
	}
	walks := map[string]map[string]string{}
	for leaf, val := range walk {
		col, idx, ok := strings.Cut(leaf, ".")
		if !ok {
			continue
		}
		field, ok := suffixToField[col]
		if !ok {
			continue
		}
		if walks[field] == nil {
			walks[field] = map[string]string{}
		}
		walks[field][idx] = val
	}
	if len(walks["INDEX"]) == 0 {
		return nil
	}
	indexes := make([]string, 0, len(walks["INDEX"]))
	for _, v := range walks["INDEX"] {
		indexes = append(indexes, strings.TrimSpace(v))
	}
	sort.Slice(indexes, func(i, j int) bool { return ifIndexLess(indexes[i], indexes[j]) })

	components := make([]map[string]any, 0, len(indexes))
	for _, idx := range indexes {
		name := idx
		if n, err := strconv.Atoi(idx); err == nil {
			name = strconv.Itoa(n - 1) // chassis number in interface names starts at 0
		}
		comp := map[string]any{
			"INDEX":            idx,
			"NAME":             name,
			"CONTAINEDININDEX": "-1",
			"TYPE":             "chassis",
		}
		for _, c := range cols {
			if v := strings.TrimSpace(walks[c.field][idx]); v != "" {
				comp[c.field] = v
			}
		}
		components = append(components, comp)
	}
	return components
}

// force10Ports builds the port components from chSysPortIfIndex; the stack unit
// is the second-to-last element of each row suffix.
func force10Ports(g SNMPGetter, base string) []map[string]any {
	walk, _ := g.Walk(base)
	if len(walk) == 0 {
		return nil
	}
	suffixes := make([]string, 0, len(walk))
	for s := range walk {
		suffixes = append(suffixes, s)
	}
	sort.Slice(suffixes, func(i, j int) bool { return ifIndexLess(walk[suffixes[i]], walk[suffixes[j]]) })

	var ports []map[string]any
	for _, suffix := range suffixes {
		stackID := elementFromEnd(suffix, 2)
		if stackID == "" {
			continue
		}
		ports = append(ports, map[string]any{
			"INDEX":            strings.TrimSpace(walk[suffix]),
			"CONTAINEDININDEX": stackID,
			"TYPE":             "port",
		})
	}
	return ports
}

// --- Panasas PanFS (cluster member serial keyed by the device IP) ---
func registerPanasas() {
	const (
		panFs            = "1.3.6.1.4.1.10159.1.3"
		panClusterName   = panFs + ".2.1.1.0"
		panClusterMgmtIP = panFs + ".2.1.2.0"
		panRepsetIPAddr  = panFs + ".2.1.3.1.2"
		panRepsetBladeSN = panFs + ".2.1.3.1.3"
	)
	registerMib(MibModule{
		Name:        "panasas-panfs",
		SysObjectID: oidMatch(panFs + ".0"),
		Serial: func(g SNMPGetter, d Device) string {
			ip, _ := d["IP"].(string)
			if ip == "" {
				ip = mibGet(g, panClusterMgmtIP)
			}
			if ip == "" {
				return ""
			}
			members, _ := g.Walk(panRepsetIPAddr)
			for index, addr := range members {
				if strings.TrimSpace(addr) == ip {
					return hex2char(mibGet(g, panRepsetBladeSN+"."+index))
				}
			}
			return ""
		},
		Run: func(g SNMPGetter, d Device) {
			if name := mibGet(g, panClusterName); name != "" {
				d["NAME"] = name
			}
		},
	})
}

// --- Siemens industrial modules (iASi-Link, sysDescr fallback) ---
func registerSiemens() {
	const (
		ad              = "1.3.6.1.4.1.4196"
		siemens         = "1.3.6.1.4.1.4329"
		iAsiLinkMib     = ad + ".1.1.8.3.100"
		snGen           = iAsiLinkMib + ".1.8"
		snTcpIp         = iAsiLinkMib + ".1.10"
		snSwVersion     = snGen + ".4.0"
		snInfoSerialNr  = snGen + ".6.0"
		snInfoMLFBNr    = snGen + ".26.0"
		snMacAddrBase   = snTcpIp + ".10.0"
		snPnioDeviceNam = iAsiLinkMib + ".2.21.2.0"
		moduleMLFB      = siemens + ".6.3.2.1.1.2.0"
		moduleSerial    = siemens + ".6.3.2.1.1.3.0"
		moduleFirmware  = siemens + ".6.3.2.1.1.5.0"
	)
	mlfbModels := map[string]string{
		"6GK1 411-2AB10":      "IE/AS-i LINK PN IO",
		"6GK7 343-1CX10-0XE0": "CP 343-1 Lean",
		"6ES7 318-3EL01-0AB0": "CPU319-3 PN/DP",
	}
	mac := func(g SNMPGetter) string {
		v := mibGet(g, snMacAddrBase)
		if v == "" {
			return ""
		}
		return canonicalMAC(v)
	}
	serial := func(g SNMPGetter, d Device) string {
		s := firstNonEmpty(mibGet(g, snInfoSerialNr), mibGet(g, moduleSerial))
		if s == "" {
			s = siemensDescrField(g, 6)
		}
		if s != "" && !strings.Contains(s, "not set") {
			return s
		}
		if m := mac(g); m != "" {
			return strings.ReplaceAll(m, ":", "")
		}
		return ""
	}
	registerMib(MibModule{
		Name: "siemens",
		// 4196 prefix, or the bad ".0.0" sysObjectID some modules report.
		SysObjectID: regexp.MustCompile(`^\.?1\.3\.6\.1\.4\.1\.4196|\.?0\.0$`),
		Priority:    20,
		Type:        func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(_ SNMPGetter, d Device) string {
			if m, _ := d["MANUFACTURER"].(string); strings.TrimSpace(m) != "" {
				return ""
			}
			return "Siemens"
		},
		Model: func(g SNMPGetter, _ Device) string {
			mlfb := firstNonEmpty(mibGet(g, snInfoMLFBNr), mibGet(g, moduleMLFB))
			if mlfb == "" {
				mlfb = siemensDescrField(g, 3)
			}
			if mlfb == "" {
				return ""
			}
			if model, ok := mlfbModels[mlfb]; ok {
				return model
			}
			return "Siemens module (PartNumber: " + mlfb + ")"
		},
		Serial: serial,
		Mac:    func(g SNMPGetter, _ Device) string { return mac(g) },
		SnmpHostname: func(g SNMPGetter, d Device) string {
			if name := mibGet(g, snPnioDeviceNam); name != "" {
				return name
			}
			return serial(g, d)
		},
		Firmware: func(g SNMPGetter, _ Device) string {
			v := firstNonEmpty(mibGet(g, snSwVersion), mibGet(g, moduleFirmware))
			if v == "" {
				v = siemensDescrMatch(g, regexp.MustCompile(`^FW: (.*)$`))
			}
			return v
		},
	})
}

// siemensDescrFields splits the sysDescr on commas (the _getInfosFromDescr base).
func siemensDescrFields(g SNMPGetter) []string {
	descr := getOne(g, oidSysDescr)
	if descr == "" {
		return nil
	}
	return regexp.MustCompile(`\s*,\s*`).Split(descr, -1)
}

// siemensDescrField returns the n-th comma-separated sysDescr field, or "".
func siemensDescrField(g SNMPGetter, n int) string {
	fields := siemensDescrFields(g)
	if n < len(fields) {
		return strings.TrimSpace(fields[n])
	}
	return ""
}

// siemensDescrMatch returns the first capture of the first sysDescr field
// matching re.
func siemensDescrMatch(g SNMPGetter, re *regexp.Regexp) string {
	for _, f := range siemensDescrFields(g) {
		if m := re.FindStringSubmatch(strings.TrimSpace(f)); m != nil {
			return m[1]
		}
	}
	return ""
}

// --- FreeBSD / Stormshield (net-snmp on FreeBSD, Stormshield private MIB) ---
func registerFreeBSD() {
	const (
		freebsd          = "1.3.6.1.4.1.8072.3.2.8"
		stormshield      = "1.3.6.1.4.1.11256"
		stormshieldModel = stormshield + ".1.0.1.0"
		stormshieldFwPri = stormshield + ".1.0.2.0"
		stormshieldSeria = stormshield + ".1.0.3.0"
		stormshieldName  = stormshield + ".1.0.4.0"
	)
	isStormshield := func(g SNMPGetter) bool { return mibGet(g, stormshieldModel) != "" }
	registerMib(MibModule{
		Name:        "FreeBSD",
		SysObjectID: oidMatch(freebsd),
		Type: func(g SNMPGetter, _ Device) string {
			if isStormshield(g) {
				return "NETWORKING"
			}
			return ""
		},
		Manufacturer: func(g SNMPGetter, _ Device) string {
			if isStormshield(g) {
				return "StormShield"
			}
			return ""
		},
		Model: func(g SNMPGetter, _ Device) string {
			if isStormshield(g) {
				return mibGet(g, stormshieldModel)
			}
			return ""
		},
		Serial: func(g SNMPGetter, _ Device) string {
			if isStormshield(g) {
				return mibGet(g, stormshieldSeria)
			}
			return ""
		},
		Firmware: func(g SNMPGetter, _ Device) string {
			if isStormshield(g) {
				return mibGet(g, stormshieldFwPri)
			}
			return ""
		},
		Run: func(g SNMPGetter, d Device) {
			if isStormshield(g) {
				if name := mibGet(g, stormshieldName); name != "" {
					d["NAME"] = name
				}
			}
		},
	})
}

// hex2char decodes a "0x"-prefixed hex string to its bytes, mirroring the Perl
// hex2char helper. Non-hex input is returned unchanged.
func hex2char(s string) string {
	s = strings.TrimSpace(s)
	if !strings.HasPrefix(s, "0x") && !strings.HasPrefix(s, "0X") {
		return s
	}
	body := s[2:]
	if len(body) == 0 || len(body)%2 != 0 {
		return s
	}
	b, err := hex.DecodeString(body)
	if err != nil {
		return s
	}
	return string(b)
}

// sortedWalkKeys returns the keys of a walk table sorted as the Perl `sort keys`
// would (lexical), so callers picking the "first" key are deterministic.
func sortedWalkKeys(walk map[string]string) []string {
	keys := make([]string, 0, len(walk))
	for k := range walk {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

// elementFromEnd returns the n-th element (1-based) from the end of a
// dot-separated OID suffix, mirroring Force10S::_getElement($oid, -n).
func elementFromEnd(oid string, n int) string {
	parts := strings.Split(oid, ".")
	if n > len(parts) {
		return ""
	}
	return parts[len(parts)-n]
}
