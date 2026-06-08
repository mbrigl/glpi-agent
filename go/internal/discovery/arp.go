// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"bufio"
	"regexp"
	"strings"
)

// HostInfo holds the non-SNMP discovery fields for one address, mirroring the
// fields NetDiscovery.pm merges from arp/ping/netbios.
type HostInfo struct {
	MAC         string
	DNSHostname string
	NetbiosName string
	Workgroup   string
}

// Any reports whether the host info carries at least one identifying field, the
// condition NetDiscovery uses to keep a non-SNMP device.
func (h HostInfo) Any() bool {
	return h.MAC != "" || h.DNSHostname != "" || h.NetbiosName != "" || h.Workgroup != ""
}

// ParseProcNetArp parses the contents of /proc/net/arp into an ip -> MAC map,
// skipping incomplete entries and the all-zero address.
func ParseProcNetArp(content string) map[string]string {
	table := map[string]string{}
	scanner := bufio.NewScanner(strings.NewReader(content))
	first := true
	for scanner.Scan() {
		if first { // header line
			first = false
			continue
		}
		f := strings.Fields(scanner.Text())
		if len(f) < 4 {
			continue
		}
		ip, flags, mac := f[0], f[2], f[3]
		if flags == "0x0" || mac == "00:00:00:00:00:00" {
			continue // incomplete entry
		}
		table[ip] = mac
	}
	return table
}

var (
	arpBSDRE   = regexp.MustCompile(`^(\S+) \(\S+\) at (\S+) `)              // host (ip) at mac
	arpWinRE   = regexp.MustCompile(`^\s+\S+\s+([0-9A-Fa-f:-]{11,})\s`)      // arp -a (dashes)
	arpNeighRE = regexp.MustCompile(`dev\s+\S+\s+lladdr\s+([0-9A-Fa-f:-]+)`) // ip neighbor
)

// ParseArpCommand extracts the MAC and (BSD form) hostname from a single-host
// `arp <ip>` / `ip neighbor show <ip>` output, mirroring _scanAddressByArp.
func ParseArpCommand(out string) (mac, hostname string) {
	out = strings.TrimRight(out, "\n")
	for _, line := range strings.Split(out, "\n") {
		if m := arpBSDRE.FindStringSubmatch(line); m != nil {
			if m[1] != "?" {
				hostname = m[1]
			}
			return canonicalMAC(m[2]), hostname
		}
		if m := arpNeighRE.FindStringSubmatch(line); m != nil {
			return canonicalMAC(m[1]), ""
		}
		if m := arpWinRE.FindStringSubmatch(line); m != nil {
			return canonicalMAC(m[1]), ""
		}
	}
	return "", ""
}

// canonicalMAC normalises a MAC address to lowercase colon-separated form.
func canonicalMAC(s string) string {
	return strings.ToLower(strings.ReplaceAll(strings.TrimSpace(s), "-", ":"))
}
