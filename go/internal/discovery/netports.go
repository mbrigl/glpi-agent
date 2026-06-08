// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"sort"
	"strconv"
	"strings"
)

// Port-enrichment OIDs from SNMP/Hardware.pm's networking properties.
const (
	oidDot1dBasePortIfIndex = "1.3.6.1.2.1.17.1.4.1.2"

	// Trunk detection.
	oidVlanTrunkPortDynStatus = "1.3.6.1.4.1.9.9.46.1.6.1.1.14"      // Cisco
	oidJnxExVlanPortAccess    = "1.3.6.1.4.1.2636.3.40.1.5.1.7.1.5"  // Juniper
	oidLldpLocPortVlanID      = "1.0.8802.1.1.2.1.5.32962.1.2.1.1.1" // others

	// Aggregation.
	oidAggPortAttachedAggID = "1.2.840.10006.300.43.1.2.1.1.13" // LACP
	oidPagpPorts            = "1.3.6.1.4.1.9.9.98.1.1.1.1.5"    // PAgP

	// Known MAC addresses (forwarding databases).
	oidDot1dTpFdbAddress = "1.3.6.1.2.1.17.4.3.1.1"
	oidDot1dTpFdbPort    = "1.3.6.1.2.1.17.4.3.1.2"
	oidDot1dTpFdbStatus  = "1.3.6.1.2.1.17.4.3.1.3"
	oidDot1qTpFdbAddress = "1.3.6.1.2.1.17.7.1.2.2.1.1"
	oidDot1qTpFdbPort    = "1.3.6.1.2.1.17.7.1.2.2.1.2"
	oidDot1qTpFdbStatus  = "1.3.6.1.2.1.17.7.1.2.2.1.3"
	oidIpNetToMediaMac   = "1.3.6.1.2.1.4.22.1.2" // ipNetToMediaPhysAddress
	oidIpNetToMediaIf    = "1.3.6.1.2.1.4.22.1.1" // ipNetToMediaIfIndex
)

// setNetworkingProperties enriches the device PORTS with the trunk flag,
// aggregation members and known (forwarding-database) MAC addresses, mirroring
// SNMP/Hardware.pm::_setNetworkingProperties. It runs only for NETWORKING
// devices. The CDP/LLDP/EDP neighbour discovery and the per-VLAN FDB context
// switching are follow-on.
func setNetworkingProperties(g SNMPGetter, device Device) {
	ports, _ := device["PORTS"].([]map[string]any)
	if len(ports) == 0 {
		return
	}
	byNum := make(map[string]map[string]any, len(ports))
	for _, p := range ports {
		if n, _ := p["IFNUMBER"].(string); n != "" {
			byNum[n] = p
		}
	}

	setTrunkPorts(g, byNum)
	setKnownMacAddresses(g, byNum)
	setAggregatePorts(g, byNum)
}

// setTrunkPorts sets PORT.TRUNK (1/0) from the first supported trunk table.
func setTrunkPorts(g SNMPGetter, byNum map[string]map[string]any) {
	for interfaceID, trunk := range getTrunkPorts(g) {
		if port := byNum[interfaceID]; port != nil {
			port["TRUNK"] = trunk
		}
	}
}

// getTrunkPorts returns interface-id -> trunk flag, trying the Cisco, then
// Juniper, then LLDP tables (the first that answers wins).
func getTrunkPorts(g SNMPGetter) map[string]int {
	results := map[string]int{}

	// Cisco vlanTrunkPortDynamicStatus: prefix.<ifIndex> = 1 (trunk) | 2 (access).
	if status, _ := g.Walk(oidVlanTrunkPortDynStatus); len(status) > 0 {
		for interfaceID, v := range status {
			results[interfaceID] = boolToInt(strings.TrimSpace(v) == "1")
		}
		return results
	}

	// Juniper jnxExVlanPortAccessMode: prefix.<vlan>.<port> = 1 (access) | 2 (trunk).
	if access, _ := g.Walk(oidJnxExVlanPortAccess); len(access) > 0 {
		port2if, _ := g.Walk(oidDot1dBasePortIfIndex)
		for suffix, v := range access {
			portID := elementFromEnd(suffix, 1)
			interfaceID := port2if[portID]
			if interfaceID == "" {
				continue
			}
			results[interfaceID] = boolToInt(strings.TrimSpace(v) == "2")
		}
		return results
	}

	// Others, lldpXdot1LocPortVlanId: prefix.<id> = vlan, 0 means trunk.
	if vlanID, _ := g.Walk(oidLldpLocPortVlanID); len(vlanID) > 0 {
		port2if, _ := g.Walk(oidDot1dBasePortIfIndex)
		for id, v := range vlanID {
			interfaceID := mapPortInterface(port2if, id)
			results[interfaceID] = boolToInt(strings.TrimSpace(v) == "0")
		}
		return results
	}

	return results
}

// setAggregatePorts sets PORT.AGGREGATE.PORT (the member interface ids) from the
// LACP then PAgP tables.
func setAggregatePorts(g SNMPGetter, byNum map[string]map[string]any) {
	apply := func(info map[string][]string) {
		for interfaceID, members := range info {
			if port := byNum[interfaceID]; port != nil {
				port["AGGREGATE"] = map[string]any{"PORT": members}
			}
		}
	}
	apply(getLACPInfo(g))
	apply(getPAGPInfo(g))
}

// getLACPInfo maps each aggregator interface to its member interfaces
// (dot3adAggPortAttachedAggID).
func getLACPInfo(g SNMPGetter) map[string][]string {
	attached, _ := g.Walk(oidAggPortAttachedAggID)
	results := map[string][]string{}
	for _, interfaceID := range sortedNumericKeys(attached) {
		aggregatorID := strings.TrimSpace(attached[interfaceID])
		if aggregatorID == "0" || aggregatorID == interfaceID {
			continue
		}
		results[aggregatorID] = append(results[aggregatorID], interfaceID)
	}
	return results
}

// getPAGPInfo maps each PAgP aggregate port (short number + 5000) to its member
// ports (pagpEthcOperationMode group ids).
func getPAGPInfo(g SNMPGetter) map[string][]string {
	pagp, _ := g.Walk(oidPagpPorts)
	results := map[string][]string{}
	for _, portID := range sortedNumericKeys(pagp) {
		short, err := strconv.Atoi(strings.TrimSpace(pagp[portID]))
		if err != nil || short <= 0 {
			continue
		}
		aggregateID := strconv.Itoa(short + 5000)
		results[aggregateID] = append(results[aggregateID], portID)
	}
	return results
}

// setKnownMacAddresses attaches forwarding-database MAC addresses to the ports
// as additional connections (default-VLAN dot1d, then dot1q, then the
// deprecated ipNetToMedia table when no dot1q entry was found).
func setKnownMacAddresses(g SNMPGetter, byNum map[string]map[string]any) {
	if addrs := getKnownMacAddresses(g, oidDot1dTpFdbAddress, oidDot1dTpFdbPort, oidDot1dTpFdbStatus); len(addrs) > 0 {
		addKnownMacAddresses(byNum, addrs)
	}

	if addrs := getKnownMacAddresses(g, oidDot1qTpFdbAddress, oidDot1qTpFdbPort, oidDot1qTpFdbStatus); len(addrs) > 0 {
		addKnownMacAddresses(byNum, addrs)
	} else if addrs := getKnownMacAddressesDeprecated(g); len(addrs) > 0 {
		// The per-VLAN FDB context switching is not supported; fall back to the
		// deprecated ipNetToMedia table as upstream does when no VLAN yielded any.
		addKnownMacAddresses(byNum, addrs)
	}
}

// getKnownMacAddresses parses a bridge forwarding database into interface-id ->
// MAC list. Only learned(3) or mgmt(5) entries are kept; when the address table
// is absent the MAC is recovered from the last six decimal bytes of the suffix.
func getKnownMacAddresses(g SNMPGetter, macOID, portOID, statusOID string) map[string][]string {
	addrs, _ := g.Walk(macOID)
	addr2port, _ := g.Walk(portOID)
	status, _ := g.Walk(statusOID)
	port2if, _ := g.Walk(oidDot1dBasePortIfIndex)

	results := map[string][]string{}
	for _, suffix := range sortedSuffixKeys(addr2port) {
		portID := strings.TrimSpace(addr2port[suffix])
		interfaceID := port2if[portID]
		if interfaceID == "" {
			continue
		}
		if raw := addrs[suffix]; raw != "" {
			mac := canonicalMAC(strings.TrimSpace(raw))
			if mac == "" {
				continue
			}
			// Assume learned(3) if no status; keep learned(3) and mgmt(5).
			st := 3
			if s, ok := status[suffix]; ok {
				st = firstInt(s, 3)
			}
			if st != 3 && st != 5 {
				continue
			}
			results[interfaceID] = append(results[interfaceID], mac)
		} else if mac := macFromSuffix(suffix); mac != "" {
			results[interfaceID] = append(results[interfaceID], mac)
		}
	}
	return results
}

// getKnownMacAddressesDeprecated reads the deprecated ipNetToMedia table.
func getKnownMacAddressesDeprecated(g SNMPGetter) map[string][]string {
	addr2mac, _ := g.Walk(oidIpNetToMediaMac)
	addr2if, _ := g.Walk(oidIpNetToMediaIf)
	results := map[string][]string{}
	for _, suffix := range sortedSuffixKeys(addr2mac) {
		interfaceID := strings.TrimSpace(addr2if[suffix])
		if interfaceID == "" {
			continue
		}
		if mac := canonicalMAC(strings.TrimSpace(addr2mac[suffix])); mac != "" {
			results[interfaceID] = append(results[interfaceID], mac)
		}
	}
	return results
}

// addKnownMacAddresses merges the discovered MACs into each port's
// CONNECTIONS.CONNECTION.MAC, skipping the port's own MAC and any already-listed
// address (and ports already identified via CDP/LLDP).
func addKnownMacAddresses(byNum map[string]map[string]any, addresses map[string][]string) {
	for portID, macs := range addresses {
		port := byNum[portID]
		if port == nil {
			continue
		}
		// A CDP/LLDP-identified connection takes precedence.
		if conns, ok := port["CONNECTIONS"].(map[string]any); ok {
			if cdp, _ := conns["CDP"].(int); cdp == 1 {
				continue
			}
		}

		existing := portConnectionMACs(port)
		known := map[string]bool{}
		for _, m := range existing {
			known[m] = true
		}
		if pm, _ := port["MAC"].(string); pm != "" {
			known[canonicalMAC(pm)] = true
		}

		merged := append([]string{}, existing...)
		for _, m := range macs {
			if !known[m] {
				known[m] = true
				merged = append(merged, m)
			}
		}
		if len(merged) == len(existing) {
			continue // nothing new
		}
		setPortConnectionMACs(port, merged)
	}
}

// portConnectionMACs returns the MAC list already stored under
// CONNECTIONS.CONNECTION.MAC, or nil.
func portConnectionMACs(port map[string]any) []string {
	conns, _ := port["CONNECTIONS"].(map[string]any)
	if conns == nil {
		return nil
	}
	conn, _ := conns["CONNECTION"].(map[string]any)
	if conn == nil {
		return nil
	}
	macs, _ := conn["MAC"].([]string)
	return macs
}

// setPortConnectionMACs stores the MAC list under CONNECTIONS.CONNECTION.MAC.
func setPortConnectionMACs(port map[string]any, macs []string) {
	conns, _ := port["CONNECTIONS"].(map[string]any)
	if conns == nil {
		conns = map[string]any{}
		port["CONNECTIONS"] = conns
	}
	conn, _ := conns["CONNECTION"].(map[string]any)
	if conn == nil {
		conn = map[string]any{}
		conns["CONNECTION"] = conn
	}
	conn["MAC"] = macs
}

// mapPortInterface resolves a bridge port id to its interface id, with the
// upstream management-port heuristic (the last port often follows the previous
// numerically and is missing from the mapping).
func mapPortInterface(port2if map[string]string, id string) string {
	if iface, ok := port2if[id]; ok {
		return iface
	}
	if n, err := strconv.Atoi(id); err == nil {
		if prev, ok := port2if[strconv.Itoa(n-1)]; ok {
			if p, err := strconv.Atoi(prev); err == nil {
				return strconv.Itoa(p + 1)
			}
		}
	}
	return id
}

// macFromSuffix builds a MAC from the last six decimal bytes of an OID suffix.
func macFromSuffix(suffix string) string {
	parts := strings.Split(suffix, ".")
	if len(parts) < 6 {
		return ""
	}
	parts = parts[len(parts)-6:]
	b := make([]byte, 6)
	for i, p := range parts {
		n, err := strconv.Atoi(p)
		if err != nil || n < 0 || n > 255 {
			return ""
		}
		b[i] = byte(n)
	}
	return hexColon(b)
}

// sortedNumericKeys returns the map keys sorted numerically (ifIndex order).
func sortedNumericKeys(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Slice(keys, func(i, j int) bool { return ifIndexLess(keys[i], keys[j]) })
	return keys
}

// sortedSuffixKeys returns the map keys sorted lexically (the Perl `sort keys`).
func sortedSuffixKeys(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

// firstInt returns the first integer found in s, or def when there is none.
func firstInt(s string, def int) int {
	start := -1
	for i, c := range s {
		if c >= '0' && c <= '9' {
			start = i
			break
		}
	}
	if start < 0 {
		return def
	}
	end := start
	for end < len(s) && s[end] >= '0' && s[end] <= '9' {
		end++
	}
	n, err := strconv.Atoi(s[start:end])
	if err != nil {
		return def
	}
	return n
}

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}
