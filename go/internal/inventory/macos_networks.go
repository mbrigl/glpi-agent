// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strconv"
	"strings"
)

// macNetSetup describes one networksetup hardware port.
type macNetSetup struct {
	description string
	macaddr     string
}

// parseMacNetworkSetup parses `networksetup -listallhardwareports` into a map
// keyed by device name, mirroring MacOS/Networks.pm _parseNetworkSetup.
func parseMacNetworkSetup(text string) map[string]macNetSetup {
	out := map[string]macNetSetup{}
	var cur macNetSetup
	for _, line := range strings.Split(text, "\n") {
		switch {
		case strings.HasPrefix(line, "Hardware Port: "):
			cur = macNetSetup{description: strings.TrimSpace(line[len("Hardware Port: "):])}
		case strings.HasPrefix(line, "Device: "):
			out[strings.TrimSpace(line[len("Device: "):])] = cur
		case strings.HasPrefix(line, "Ethernet Address: "):
			cur.macaddr = strings.TrimSpace(line[len("Ethernet Address: "):])
		case strings.HasPrefix(line, "VLAN Configurations"):
			return out
		}
	}
	return out
}

var (
	macIfaceRE   = regexp.MustCompile(`^(\S+):`)
	macInetRE    = regexp.MustCompile(`inet (\d+\.\d+\.\d+\.\d+)`)
	macInet6RE   = regexp.MustCompile(`inet6 (\S+)`)
	macNetmaskRE = regexp.MustCompile(`netmask 0x([0-9a-fA-F]{8})`)
	macMacRE     = regexp.MustCompile(`(?:address:|ether|lladdr) ([0-9a-fA-F]{2}(?::[0-9a-fA-F]{2}){5})`)
	macMTURE     = regexp.MustCompile(`mtu (\S+)`)
	macMediaRE   = regexp.MustCompile(`media (\S+)`)
	macSpeedRE   = regexp.MustCompile(`media: \S+ \((\d+)baseTX <.*>\)`)
	macStatusRE  = regexp.MustCompile(`(?i)status:\s+active`)
)

// buildMacNetworks parses `ifconfig -a` (joined with the networksetup hardware
// ports) into the NETWORKS section, mirroring MacOS/Networks.pm _parseIfconfig +
// _getInterfaces: per-interface STATUS/DESCRIPTION/VIRTUALDEV/MACADDR/TYPE/
// IPADDRESS/IPADDRESS6/IPMASK/IPSUBNET/MTU/SPEED. The routing-table IPGATEWAY /
// hardware DEFAULTGATEWAY are added live by the collector.
func buildMacNetworks(ifconfig string, netsetup map[string]macNetSetup) []map[string]any {
	var interfaces []map[string]any
	var cur map[string]any

	flush := func() {
		if cur == nil {
			return
		}
		ip, _ := cur["IPADDRESS"].(string)
		mask, _ := cur["IPMASK"].(string)
		if ip != "" && mask != "" {
			if subnet := subnetAddress(ip, mask); subnet != "" {
				cur["IPSUBNET"] = subnet
			}
		}
		interfaces = append(interfaces, cur)
	}

	for _, line := range strings.Split(ifconfig, "\n") {
		if m := macIfaceRE.FindStringSubmatch(line); m != nil {
			flush()
			name := m[1]
			ns, known := netsetup[name]
			desc := name
			virtual := 1
			if known {
				virtual = 0
				if ns.description != "" {
					desc = ns.description
				}
			}
			cur = map[string]any{
				"STATUS":      "Down",
				"DESCRIPTION": desc,
				"VIRTUALDEV":  virtual,
			}
			if known && ns.macaddr != "" {
				cur["MACADDR"] = ns.macaddr
			}
			if t := macInterfaceType(desc); t != "" {
				cur["TYPE"] = t
			}
		}
		if cur == nil {
			continue
		}

		if m := macInetRE.FindStringSubmatch(line); m != nil {
			cur["IPADDRESS"] = m[1]
		}
		if m := macInet6RE.FindStringSubmatch(line); m != nil {
			cur["IPADDRESS6"] = stripZoneID(m[1])
		}
		if m := macNetmaskRE.FindStringSubmatch(line); m != nil {
			cur["IPMASK"] = hexToDottedMask(m[1])
		}
		if m := macMacRE.FindStringSubmatch(line); m != nil {
			cur["MACADDR"] = m[1]
		}
		if m := macMTURE.FindStringSubmatch(line); m != nil {
			cur["MTU"] = atoiOr(m[1], 0)
		}
		if m := macMediaRE.FindStringSubmatch(line); m != nil {
			if _, ok := cur["TYPE"]; !ok {
				cur["TYPE"] = m[1]
			}
		}
		if m := macSpeedRE.FindStringSubmatch(line); m != nil {
			cur["SPEED"] = atoiOr(m[1], 0)
		}
		if macStatusRE.MatchString(line) {
			cur["STATUS"] = "Up"
		}
		if strings.Contains(line, "supported  media:") || strings.Contains(line, "supported media:") {
			cur["VIRTUALDEV"] = 0
		}
	}
	flush()
	return interfaces
}

// macInterfaceType classifies an interface from its description, mirroring the
// MacOS/Networks.pm port-type heuristics.
func macInterfaceType(desc string) string {
	switch {
	case regexp.MustCompile(`^lo\d+$`).MatchString(desc):
		return "loopback"
	case regexp.MustCompile(`(?i)bridge`).MatchString(desc):
		return "bridge"
	case regexp.MustCompile(`(?i)wi-?fi`).MatchString(desc):
		return "wifi"
	case regexp.MustCompile(`(?i)bluetooth`).MatchString(desc):
		return "bluetooth"
	case regexp.MustCompile(`(?i)phone`).MatchString(desc):
		return "dialup"
	case regexp.MustCompile(`(?i)ethernet|thunderbolt|usb.*lan`).MatchString(desc):
		return "ethernet"
	}
	return ""
}

// hexToDottedMask converts an 8-hex-digit netmask ("ffffff00") to dotted form
// ("255.255.255.0"), mirroring hex2canonical.
func hexToDottedMask(hexMask string) string {
	octets := make([]string, 4)
	for i := 0; i < 4; i++ {
		v, _ := strconv.ParseUint(hexMask[i*2:i*2+2], 16, 8)
		octets[i] = strconv.FormatUint(v, 10)
	}
	return strings.Join(octets, ".")
}
