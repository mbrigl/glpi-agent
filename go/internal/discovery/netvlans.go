// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"sort"
	"strconv"
	"strings"
)

// VLAN-membership OIDs (SNMP/Hardware.pm::_getVlans).
const (
	oidVtpVlanName  = "1.3.6.1.4.1.9.9.46.1.3.1.1.4.1" // Cisco VTP vlan names
	oidVmPortStatus = "1.3.6.1.4.1.9.9.68.1.2.2.1.2"   // Cisco per-port vlan id

	oidDot1qVlanStaticName          = "1.3.6.1.2.1.17.7.1.4.3.1.1"
	oidDot1qVlanStaticEgressPorts   = "1.3.6.1.2.1.17.7.1.4.3.1.2"
	oidDot1qVlanStaticUntaggedPorts = "1.3.6.1.2.1.17.7.1.4.3.1.4"
	oidDot1qVlanStaticRowStatus     = "1.3.6.1.2.1.17.7.1.4.3.1.5"
	oidDot1qVlanCurrentEgressPorts  = "1.3.6.1.2.1.17.7.1.4.2.1.4"
	oidDot1qVlanCurrentUntagPorts   = "1.3.6.1.2.1.17.7.1.4.2.1.5"

	oidLldpVlanIdName = "1.0.8802.1.1.2.1.5.32962.1.2.3.1.2" // lldpXdot1LocVlanName
	oidLldpLocPortId  = "1.0.8802.1.1.2.1.3.7.1.3"           // lldpLocPortId
	oidLldpVlanId     = "1.0.8802.1.1.2.1.5.32962.1.2.1.1.1" // lldpXdot1LocPortVlanId
)

// setVlans attaches PORT.VLANS.VLAN to each port, mirroring
// SNMP/Hardware.pm::_setVlans + _getVlans: the resulting per-port-id VLAN lists
// are remapped to interface ids via dot1dBasePortIfIndex when needed.
func setVlans(g SNMPGetter, byNum map[string]map[string]any) {
	vlans := getVlans(g, byNum)
	if len(vlans) == 0 {
		return
	}
	port2if, _ := g.Walk(oidDot1dBasePortIfIndex)

	for portID, vlanList := range vlans {
		target := portID
		if _, ok := byNum[portID]; !ok {
			// Extreme-style LLDP indexes: remap through dot1dBasePortIfIndex.
			if mapped := port2if[portID]; mapped != "" && byNum[mapped] != nil {
				target = mapped
			} else {
				continue
			}
		}
		port := byNum[target]
		port["VLANS"] = map[string]any{"VLAN": vlanList}
	}
}

// getVlans returns interface-id -> []VLAN, trying the Cisco VTP table, the
// dot1q static/current tables (egress/untagged bitmaps), then the LLDP fallbacks.
func getVlans(g SNMPGetter, byNum map[string]map[string]any) map[string][]map[string]any {
	results := map[string][]map[string]any{}

	// Cisco: per-port vlan id (vmPortStatus) named via vtpVlanName.
	vtpName, _ := g.Walk(oidVtpVlanName)
	vmPort, _ := g.Walk(oidVmPortStatus)
	if len(vtpName) > 0 && len(vmPort) > 0 {
		for _, suffix := range sortedKeys(vmPort) {
			portID := elementFromEnd(suffix, 1)
			vlanID := strings.TrimSpace(vmPort[suffix])
			results[portID] = append(results[portID], map[string]any{
				"NUMBER": vlanID,
				"NAME":   strings.TrimSpace(vtpName[vlanID]),
			})
		}
	}

	// 802.1Q static/current egress + untagged bitmaps.
	getDot1qVlans(g, byNum, results)

	if len(results) == 0 {
		getLldpVlans(g, results)
	}
	return results
}

// getDot1qVlans fills the per-port VLAN list from the dot1q egress/untagged
// port bitmaps (SNMP/Hardware.pm dot1qVlanStatic/Current path).
func getDot1qVlans(g SNMPGetter, byNum map[string]map[string]any, results map[string][]map[string]any) {
	staticName, _ := g.Walk(oidDot1qVlanStaticName)
	rowStatus, _ := g.Walk(oidDot1qVlanStaticRowStatus)
	if len(staticName) == 0 || len(rowStatus) == 0 {
		return
	}
	staticEgress, _ := g.Walk(oidDot1qVlanStaticEgressPorts)
	staticUntag, _ := g.Walk(oidDot1qVlanStaticUntaggedPorts)
	currentEgress, _ := g.Walk(oidDot1qVlanCurrentEgressPorts)
	currentUntag, _ := g.Walk(oidDot1qVlanCurrentUntagPorts)

	for _, vlanID := range sortedKeys(rowStatus) {
		if strings.TrimSpace(rowStatus[vlanID]) != "1" {
			continue
		}
		name := strings.TrimSpace(staticName[vlanID])

		// Egress ports: prefer the "current" table (keyed by a vlan suffix),
		// fall back to the static one.
		suffix, egress := lookupVlanPorts(currentEgress, vlanID)
		if egress == "" {
			if v, ok := staticEgress[vlanID]; ok {
				egress = v
			} else {
				continue
			}
		}
		bEgress := octetBits(egress)
		if bEgress == "" {
			continue
		}

		var untagged string
		if suffix != "" {
			untagged = currentUntag[suffix]
		}
		if untagged == "" {
			if v, ok := staticUntag[vlanID]; ok {
				untagged = v
			} else {
				continue
			}
		}
		bUntag := octetBits(untagged)
		if bUntag == "" {
			continue
		}

		for portID := range byNum {
			pn, err := strconv.Atoi(portID)
			if err != nil || pn < 1 || pn > len(bEgress) || pn > len(bUntag) {
				continue
			}
			isUntagged := bUntag[pn-1] == '1'
			isTagged := !isUntagged && bEgress[pn-1] == '1'
			if isTagged || isUntagged {
				results[portID] = append(results[portID], map[string]any{
					"NUMBER": vlanID,
					"NAME":   name,
					"TAGGED": boolToInt(isTagged),
				})
			}
		}
	}
}

// getLldpVlans fills VLANs from the LLDP vlan-name table, then the per-port
// vlan-id table (Alcatel/Extreme path).
func getLldpVlans(g SNMPGetter, results map[string][]map[string]any) {
	vlanIDName, _ := g.Walk(oidLldpVlanIdName)
	portLink, _ := g.Walk(oidLldpLocPortId)
	if len(vlanIDName) > 0 && len(portLink) > 0 {
		for _, suffix := range sortedKeys(vlanIDName) {
			parts := strings.SplitN(suffix, ".", 2)
			port := parts[0]
			vlan := ""
			if len(parts) > 1 {
				vlan = parts[1]
			}
			link := portLink[port]
			if link == "" {
				continue
			}
			portNumber := link
			if !isAllDigits(link) {
				portNumber = port
			}
			results[portNumber] = append(results[portNumber], map[string]any{
				"NUMBER": vlan,
				"NAME":   strings.TrimSpace(vlanIDName[suffix]),
			})
		}
		return
	}

	vlanID, _ := g.Walk(oidLldpVlanId)
	for _, port := range sortedKeys(vlanID) {
		v := strings.TrimSpace(vlanID[port])
		results[port] = append(results[port], map[string]any{
			"NUMBER": v,
			"NAME":   "VLAN " + v,
		})
	}
}

// lookupVlanPorts finds the egress-ports entry for a vlan id in a "current"
// table whose suffix may be "<vlan>", "0.<vlan>" or "<x>.<vlan>".
func lookupVlanPorts(table map[string]string, vlanID string) (suffix, value string) {
	if v, ok := table[vlanID]; ok {
		return vlanID, v
	}
	if v, ok := table["0."+vlanID]; ok {
		return "0." + vlanID, v
	}
	for s, v := range table {
		if strings.HasSuffix(s, "."+vlanID) {
			return s, v
		}
	}
	return "", ""
}

// octetBits expands an octet-string bitmap (colon-hex from SNMP, or a raw
// string) into a "0"/"1" string, MSB first.
func octetBits(value string) string {
	var raw []byte
	if strings.Contains(value, ":") {
		for _, h := range strings.Split(value, ":") {
			n, err := strconv.ParseUint(strings.TrimSpace(h), 16, 8)
			if err != nil {
				return ""
			}
			raw = append(raw, byte(n))
		}
	} else {
		raw = []byte(value)
	}
	var b strings.Builder
	for _, by := range raw {
		for i := 7; i >= 0; i-- {
			if by&(1<<uint(i)) != 0 {
				b.WriteByte('1')
			} else {
				b.WriteByte('0')
			}
		}
	}
	return b.String()
}

func isAllDigits(s string) bool {
	if s == "" {
		return false
	}
	for _, r := range s {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

// sortedKeys returns map keys sorted (numerically where possible).
func sortedKeys(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Slice(keys, func(i, j int) bool { return ifIndexLess(keys[i], keys[j]) })
	return keys
}
