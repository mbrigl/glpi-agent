// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// Neighbour-discovery OIDs (SNMP/Hardware.pm CDP/LLDP/EDP getters).
const (
	// LLDP-MIB.
	oidLldpRemChassisIdSub = "1.0.8802.1.1.2.1.4.1.1.4"
	oidLldpRemChassisId    = "1.0.8802.1.1.2.1.4.1.1.5"
	oidLldpRemPortIdSubt   = "1.0.8802.1.1.2.1.4.1.1.6"
	oidLldpRemPortId       = "1.0.8802.1.1.2.1.4.1.1.7"
	oidLldpRemPortDesc     = "1.0.8802.1.1.2.1.4.1.1.8"
	oidLldpRemSysName      = "1.0.8802.1.1.2.1.4.1.1.9"
	oidLldpRemSysDesc      = "1.0.8802.1.1.2.1.4.1.1.10"
	oidCiscoPortIfIndex    = "1.3.6.1.4.1.9.5.1.4.1.1.11.1"

	// Cisco CDP-MIB.
	oidCdpCacheAddress    = "1.3.6.1.4.1.9.9.23.1.2.1.1.4"
	oidCdpCacheVersion    = "1.3.6.1.4.1.9.9.23.1.2.1.1.5"
	oidCdpCacheDeviceId   = "1.3.6.1.4.1.9.9.23.1.2.1.1.6"
	oidCdpCacheDevicePort = "1.3.6.1.4.1.9.9.23.1.2.1.1.7"
	oidCdpCachePlatform   = "1.3.6.1.4.1.9.9.23.1.2.1.1.8"
	oidCdpCacheSysName    = "1.3.6.1.4.1.9.9.23.1.2.1.1.17"

	// Extreme EDP.
	oidEdpNeighVlanIP = "1.3.6.1.4.1.1916.1.13.3.1.3"
	oidEdpNeighName   = "1.3.6.1.4.1.1916.1.13.2.1.3"
	oidEdpNeighPort   = "1.3.6.1.4.1.1916.1.13.2.1.6"
)

var macLikeRE = regexp.MustCompile(`(?i)^[0-9a-f]{2}(:[0-9a-f]{2}){5}$`)

// setConnectedDevices attaches PORT.CONNECTIONS from the LLDP, then CDP, then EDP
// neighbour tables, mirroring SNMP/Hardware.pm::_setConnectedDevices (CDP/EDP
// reconcile with an existing LLDP neighbour, otherwise replace it).
func setConnectedDevices(g SNMPGetter, byNum map[string]map[string]any) {
	for ifID, conn := range getLLDPInfo(g, byNum) {
		if port := byNum[ifID]; port != nil {
			port["CONNECTIONS"] = map[string]any{"CDP": 1, "CONNECTION": conn}
		}
	}

	for ifID, cdp := range getCDPInfo(g) {
		port := byNum[ifID]
		if port == nil {
			continue
		}
		if m, _ := cdp["MODEL"].(string); strings.HasPrefix(strings.ToLower(m), "communicator") {
			continue
		}
		mergeNeighbour(port, cdp, []string{"IP", "MODEL"})
	}

	for ifID, edp := range getEDPInfo(g) {
		if port := byNum[ifID]; port != nil {
			mergeNeighbour(port, edp, []string{"IP"})
		}
	}
}

// mergeNeighbour reconciles a CDP/EDP connection with an existing LLDP one: on a
// SYSDESCR/SYSNAME/SYSMAC match the named keys are copied onto the LLDP
// connection, on a mismatch the (ambiguous) connection is dropped, and with no
// existing LLDP connection the new one is installed.
func mergeNeighbour(port, conn map[string]any, copyKeys []string) {
	connsAny, hasConns := port["CONNECTIONS"].(map[string]any)
	var lldp map[string]any
	if hasConns {
		lldp, _ = connsAny["CONNECTION"].(map[string]any)
	}
	if lldp != nil {
		if neighbourMatch(lldp, conn) {
			for _, k := range copyKeys {
				if v, ok := conn[k]; ok {
					lldp[k] = v
				}
			}
		} else {
			delete(port, "CONNECTIONS")
		}
		return
	}
	port["CONNECTIONS"] = map[string]any{"CDP": 1, "CONNECTION": conn}
}

// neighbourMatch reports whether a CDP/EDP connection describes the same device
// as the LLDP one (matching SYSDESCR, SYSNAME, or SYSMAC).
func neighbourMatch(lldp, other map[string]any) bool {
	if s := connStr(lldp, "SYSDESCR"); s != "" && s == connStr(other, "SYSDESCR") {
		return true
	}
	if ln, on := connStr(lldp, "SYSNAME"), connStr(other, "SYSNAME"); ln != "" && on != "" {
		if ln == on {
			return true
		}
		if lm := connStr(lldp, "SYSMAC"); lm != "" && lm == canonicalMAC(on) {
			return true
		}
	}
	if lm, om := connStr(lldp, "SYSMAC"), connStr(other, "SYSMAC"); lm != "" && strings.EqualFold(lm, om) {
		return true
	}
	return false
}

// getLLDPInfo builds the per-interface LLDP connection map.
func getLLDPInfo(g SNMPGetter, byNum map[string]map[string]any) map[string]map[string]any {
	chassis, _ := g.Walk(oidLldpRemChassisId)
	if len(chassis) == 0 {
		return nil
	}
	chassisSub, _ := g.Walk(oidLldpRemChassisIdSub)
	portIdSub, _ := g.Walk(oidLldpRemPortIdSubt)
	portId, _ := g.Walk(oidLldpRemPortId)
	portDesc, _ := g.Walk(oidLldpRemPortDesc)
	sysName, _ := g.Walk(oidLldpRemSysName)
	sysDesc, _ := g.Walk(oidLldpRemSysDesc)

	port2if := firstWalk(g, oidCiscoPortIfIndex, oidDot1dBasePortIfIndex)

	results := map[string]map[string]any{}
	suffixes := make([]string, 0, len(chassis))
	for s := range chassis {
		suffixes = append(suffixes, s)
	}
	sort.Slice(suffixes, func(i, j int) bool { return chassisIdSuffixLess(suffixes[i], suffixes[j]) })

	for _, suffix := range suffixes {
		sd := strings.TrimSpace(sysDesc[suffix])
		sn := strings.TrimSpace(sysName[suffix])
		if sd == "" && sn == "" {
			continue
		}
		// Skip unsupported suffix format and non-macAddress chassis subtypes.
		if isAllDigits(suffix) {
			continue
		}
		if sub, ok := chassisSub[suffix]; ok && strings.TrimSpace(sub) != "4" {
			continue
		}

		conn := map[string]any{"SYSMAC": canonicalMAC(chassis[suffix])}
		if sd != "" {
			conn["SYSDESCR"] = sd
		}
		if sn != "" {
			conn["SYSNAME"] = sn
		}

		pid := strings.TrimSpace(portId[suffix])
		applyLLDPPortId(conn, strings.TrimSpace(portIdSub[suffix]), pid)

		if pd := strings.TrimSpace(portDesc[suffix]); pd != "" {
			if isAllDigits(pd) && conn["IFNUMBER"] == nil {
				conn["IFNUMBER"] = pd
			} else if conn["IFDESCR"] == nil {
				conn["IFDESCR"] = pd
			}
		}

		id := elementFromEnd(suffix, 2)
		ifID := id
		if mapped := port2if[id]; mapped != "" {
			ifID = mapped
		}
		results[ifID] = conn
	}
	return results
}

// applyLLDPPortId sets MAC/IFNUMBER/IFDESCR on the connection from the remote
// port id and its subtype (LLDP PortIdSubtype textual convention).
func applyLLDPPortId(conn map[string]any, subtype, portID string) {
	sysmac, _ := conn["SYSMAC"].(string)
	addMAC := func(mac string) {
		if mac != "" && mac != sysmac {
			conn["MAC"] = []string{mac}
		}
	}
	switch subtype {
	case "3": // mac address
		addMAC(canonicalMAC(portID))
	case "1", "5", "7": // interface alias / name / local
		if isAllDigits(portID) {
			conn["IFNUMBER"] = portID
		} else if portID != "" {
			conn["IFDESCR"] = portID
		}
	default: // unknown subtype: guess
		if macLikeRE.MatchString(portID) {
			addMAC(canonicalMAC(portID))
		} else if isAllDigits(portID) {
			conn["IFNUMBER"] = portID
		} else if portID != "" {
			conn["IFDESCR"] = portID
		}
	}
}

// getCDPInfo builds the per-interface CDP connection map, dropping interfaces
// that announce multiple neighbours.
func getCDPInfo(g SNMPGetter) map[string]map[string]any {
	address, _ := g.Walk(oidCdpCacheAddress)
	if len(address) == 0 {
		return nil
	}
	version, _ := g.Walk(oidCdpCacheVersion)
	deviceId, _ := g.Walk(oidCdpCacheDeviceId)
	devicePort, _ := g.Walk(oidCdpCacheDevicePort)
	platform, _ := g.Walk(oidCdpCachePlatform)
	sysName, _ := g.Walk(oidCdpCacheSysName)

	results := map[string]map[string]any{}
	blacklist := map[string]bool{}
	for suffix, addr := range address {
		ifID := elementFromEnd(suffix, 2)
		ip := colonHexToIP(addr)
		if ip == "" || ip == "0.0.0.0" {
			continue
		}
		sd := strings.TrimSpace(version[suffix])
		model := strings.TrimSpace(platform[suffix])
		if sd == "" || model == "" {
			continue
		}
		conn := map[string]any{"IP": ip, "SYSDESCR": sd, "MODEL": model}

		dp := strings.TrimSpace(devicePort[suffix])
		if isAllDigits(dp) {
			conn["IFNUMBER"] = dp
		} else if dp != "" {
			conn["IFDESCR"] = dp
		}
		if sn := strings.TrimSpace(sysName[suffix]); sn != "" {
			conn["SYSNAME"] = sn
		}
		if did := strings.TrimSpace(deviceId[suffix]); did != "" && conn["SYSNAME"] == nil {
			if macLikeRE.MatchString(did) {
				conn["SYSMAC"] = canonicalMAC(did)
			} else {
				conn["SYSNAME"] = did
			}
		}

		if _, exists := results[ifID]; exists {
			blacklist[ifID] = true
		} else {
			results[ifID] = conn
		}
	}
	for ifID := range blacklist {
		delete(results, ifID)
	}
	return results
}

// getEDPInfo builds the per-interface Extreme EDP connection map.
func getEDPInfo(g SNMPGetter) map[string]map[string]any {
	vlanIP, _ := g.Walk(oidEdpNeighVlanIP)
	if len(vlanIP) == 0 {
		return nil
	}
	name, _ := g.Walk(oidEdpNeighName)
	port, _ := g.Walk(oidEdpNeighPort)

	results := map[string]map[string]any{}
	blacklist := map[string]bool{}
	for suffix, ip := range vlanIP {
		ip = strings.TrimSpace(ip)
		if ip == "" || ip == "0.0.0.0" {
			continue
		}
		parts := strings.Split(suffix, ".")
		if len(parts) < 9 {
			continue
		}
		ifID := parts[0]
		macParts := parts[3:9]
		shortSuffix := strings.Join(append([]string{ifID, "0", "0"}, macParts...), ".")

		conn := map[string]any{
			"IP":      ip,
			"IFDESCR": strings.TrimSpace(port[shortSuffix]),
			"SYSNAME": strings.TrimSpace(name[shortSuffix]),
			"SYSMAC":  decimalBytesToMAC(macParts),
		}
		if _, exists := results[ifID]; exists {
			blacklist[ifID] = true
		} else {
			results[ifID] = conn
		}
	}
	for ifID := range blacklist {
		delete(results, ifID)
	}
	return results
}

// colonHexToIP converts a 4-byte colon-hex octet string ("0a:0b:0c:0d") to a
// dotted IPv4 address.
func colonHexToIP(s string) string {
	s = strings.TrimSpace(s)
	if !strings.Contains(s, ":") {
		// Already dotted or empty.
		if strings.Count(s, ".") == 3 {
			return s
		}
		return ""
	}
	hexes := strings.Split(s, ":")
	if len(hexes) != 4 {
		return ""
	}
	octets := make([]string, 4)
	for i, h := range hexes {
		n, err := strconv.ParseUint(h, 16, 8)
		if err != nil {
			return ""
		}
		octets[i] = strconv.FormatUint(n, 10)
	}
	return strings.Join(octets, ".")
}

// decimalBytesToMAC joins six decimal octet strings into a colon MAC.
func decimalBytesToMAC(parts []string) string {
	if len(parts) != 6 {
		return ""
	}
	out := make([]string, 6)
	for i, p := range parts {
		n, err := strconv.Atoi(p)
		if err != nil || n < 0 || n > 255 {
			return ""
		}
		out[i] = strconv.FormatInt(int64(n), 16)
		if len(out[i]) == 1 {
			out[i] = "0" + out[i]
		}
	}
	return strings.Join(out, ":")
}

// chassisIdSuffixLess sorts LLDP suffixes by the 3rd, then 2nd, then 1st element
// numerically (SNMP/Hardware.pm::_sortChassisIdSuffix).
func chassisIdSuffixLess(a, b string) bool {
	pa := strings.Split(a, ".")
	pb := strings.Split(b, ".")
	for _, idx := range []int{2, 1, 0} {
		if idx < len(pa) && idx < len(pb) {
			ai, _ := strconv.Atoi(pa[idx])
			bi, _ := strconv.Atoi(pb[idx])
			if ai != bi {
				return ai < bi
			}
		}
	}
	return a < b
}

func connStr(m map[string]any, key string) string {
	s, _ := m[key].(string)
	return s
}
