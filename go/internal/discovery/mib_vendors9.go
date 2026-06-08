// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"regexp"
	"sort"
	"strings"
)

// Ninth batch: the last two upstream SNMP/MibSupport/* modules, which enrich
// rather than identify — CiscoPortSecurity (per-port secure MACs as PORT
// connections) and IEEE802dot11 (fill empty identity from the dot11 resource
// table). Ported verbatim from the Perl OIDs.

func init() {
	registerCiscoPortSecurity()
	registerIEEE802dot11()
	registerSnmpFramework()
}

// --- SNMP-FRAMEWORK-MIB (generic snmpEngineID fallback classifier) ---
// Priority 100 (last resort): fills MANUFACTURER/MODEL/SERIAL from the IANA
// manufacturer id decoded out of the snmpEngineID, only when nothing more
// specific (generic classification or a vendor module) provided them.
func registerSnmpFramework() {
	// snmpFrameworkMIBCompliance, advertised in the sysORID table.
	const snmpFrameworkCompliance = "1.3.6.1.6.3.10.3.1.1"
	registerMib(MibModule{
		Name:     "snmp-framework",
		OID:      snmpFrameworkCompliance,
		Priority: 100,
		Manufacturer: func(g SNMPGetter, d Device) string {
			if s, _ := d["MANUFACTURER"].(string); strings.TrimSpace(s) != "" {
				return ""
			}
			info, _, ok := snmpEngineIDInfo(g)
			if !ok {
				return ""
			}
			return info.Manufacturer
		},
		Model: func(g SNMPGetter, d Device) string {
			if s, _ := d["MODEL"].(string); strings.TrimSpace(s) != "" {
				return ""
			}
			info, _, ok := snmpEngineIDInfo(g)
			if !ok {
				return ""
			}
			return info.Model
		},
		Serial: func(g SNMPGetter, d Device) string {
			if s, _ := d["SERIAL"].(string); strings.TrimSpace(s) != "" {
				return ""
			}
			_, serial, ok := snmpEngineIDInfo(g)
			if !ok || serial == "" {
				return ""
			}
			// An entity- or printer-MIB serial still takes precedence.
			if s := firstNonEmpty(walkFirst(g, oidEntPhysicalSerialNum), walkFirst(g, oidPrtGeneralSerialNum)); s != "" {
				return s
			}
			return serial
		},
	})
}

// --- Cisco Port Security (CISCO-PORT-SECURITY-MIB) ---
func registerCiscoPortSecurity() {
	const (
		ciscoPortSecurityMIB   = "1.3.6.1.4.1.9.9.315"
		cpsGlobalEnable        = ciscoPortSecurityMIB + ".1.1.3.0"
		cpsIfSecureLastMacAddr = ciscoPortSecurityMIB + ".1.2.1.1.10"
	)
	registerMib(MibModule{
		Name:       "cisco-port-security",
		PrivateOID: cpsGlobalEnable,
		Priority:   5,
		Run: func(g SNMPGetter, d Device) {
			// Nothing to do unless the feature is enabled globally.
			if mibGet(g, cpsGlobalEnable) == "" {
				return
			}
			lastMacs, _ := g.Walk(cpsIfSecureLastMacAddr)
			if len(lastMacs) == 0 {
				return
			}
			ports, _ := d["PORTS"].([]map[string]any)
			if len(ports) == 0 {
				return
			}
			for port, raw := range lastMacs {
				mac := canonicalMAC(strings.TrimSpace(raw))
				if mac == "" {
					continue
				}
				for _, p := range ports {
					if num, _ := p["IFNUMBER"].(string); num == port {
						p["CONNECTIONS"] = map[string]any{
							"CONNECTION": map[string]any{"MAC": []string{mac}},
						}
						break
					}
				}
			}
		},
	})
}

// --- IEEE802.11 resource info (IEEE802dot11-MIB) ---
// Low priority (50): only fills MANUFACTURER/MODEL/FIRMWARE when the generic
// classification left them empty.
func registerIEEE802dot11() {
	const (
		ieee802dot11 = "1.2.840.10036"
		dot11Entry   = ieee802dot11 + ".3.1.2.1"
		dot11Manu    = dot11Entry + ".2"
		dot11Product = dot11Entry + ".3"
		dot11Version = dot11Entry + ".4"
	)
	// firstBySuffix returns the value at the lowest table suffix (sorted
	// component-wise like the Perl _getFirstKey/_sortSuffix).
	firstBySuffix := func(walk map[string]string) string {
		if len(walk) == 0 {
			return ""
		}
		keys := make([]string, 0, len(walk))
		for k := range walk {
			keys = append(keys, k)
		}
		sort.Slice(keys, func(i, j int) bool { return sortSuffix(keys[i], keys[j]) < 0 })
		return strings.TrimSpace(walk[keys[0]])
	}
	ubntVersionRE := regexp.MustCompile(`^WA\.\w+\.(v\d+\.\d+\.\d+)`)
	registerMib(MibModule{
		Name:     "ieee802dot11",
		OID:      ieee802dot11,
		Priority: 50,
		Manufacturer: func(g SNMPGetter, d Device) string {
			if s, _ := d["MANUFACTURER"].(string); strings.TrimSpace(s) != "" {
				return ""
			}
			walk, _ := g.Walk(dot11Manu)
			return firstBySuffix(walk)
		},
		Model: func(g SNMPGetter, d Device) string {
			if s, _ := d["MODEL"].(string); strings.TrimSpace(s) != "" {
				return ""
			}
			walk, _ := g.Walk(dot11Product)
			return firstBySuffix(walk)
		},
		Firmware: func(g SNMPGetter, d Device) string {
			if s, _ := d["FIRMWARE"].(string); strings.TrimSpace(s) != "" {
				return ""
			}
			walk, _ := g.Walk(dot11Version)
			version := firstBySuffix(walk)
			if version == "" {
				return ""
			}
			// Extract the Ubnt-style version when present.
			if m := ubntVersionRE.FindStringSubmatch(version); m != nil {
				return m[1] + " (WA)"
			}
			return version
		},
	})
}

// sortSuffix compares two dot-separated OID suffixes component-wise as integers,
// the shorter sorting first when one is a prefix of the other. It is a sane,
// consistent form of IEEE802dot11's _sortSuffix, which here only ever sees
// single-component (numeric) suffixes. It returns <0, 0 or >0.
func sortSuffix(a, b string) int {
	ak := strings.Split(a, ".")
	bk := strings.Split(b, ".")
	for i := 0; i < len(ak); i++ {
		bv := 0
		if i < len(bk) {
			bv = atoiSafe(bk[i])
		}
		if c := atoiSafe(ak[i]) - bv; c != 0 {
			return c
		}
	}
	if len(bk) > len(ak) {
		return -1 // a is a shorter prefix of b -> a first
	}
	return 0
}
