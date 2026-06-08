// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"math/rand"
	"net"
	"os"
	"time"
)

// NewHostProbe builds a HostProbe that discovers non-SNMP hosts the way
// NetDiscovery.pm does outside the SNMP probe: it reads the local ARP cache
// (/proc/net/arp, populated by the kernel) for the MAC of each address and
// sends a NetBIOS adapter-status (NBSTAT) query to UDP/137 for the NetBIOS
// name, workgroup and adapter MAC. timeout bounds each NBSTAT query; a
// non-positive timeout disables the NetBIOS probe.
func NewHostProbe(timeout time.Duration) HostProbe {
	arp := map[string]string{}
	if content, err := os.ReadFile("/proc/net/arp"); err == nil {
		arp = ParseProcNetArp(string(content))
	}
	return func(ip string) HostInfo {
		var h HostInfo
		if mac := arp[ip]; mac != "" {
			h.MAC = canonicalMAC(mac)
		}
		if timeout > 0 {
			if nb, err := nbstatQuery(ip, timeout); err == nil {
				if h.MAC == "" {
					h.MAC = nb.MAC
				}
				h.NetbiosName = nb.NetbiosName
				h.Workgroup = nb.Workgroup
			}
		}
		return h
	}
}

// nbstatQuery sends one NBSTAT request to ip:137 and parses the reply,
// mirroring _scanAddressByNetbios. It returns an error on timeout or a
// malformed response.
func nbstatQuery(ip string, timeout time.Duration) (HostInfo, error) {
	var h HostInfo
	conn, err := net.DialTimeout("udp", net.JoinHostPort(ip, "137"), timeout)
	if err != nil {
		return h, err
	}
	defer conn.Close()

	txid := uint16(rand.Intn(0x10000))
	if _, err := conn.Write(BuildNBStatRequest(txid)); err != nil {
		return h, err
	}
	if err := conn.SetReadDeadline(time.Now().Add(timeout)); err != nil {
		return h, err
	}
	buf := make([]byte, 1024)
	n, err := conn.Read(buf)
	if err != nil {
		return h, err
	}
	return ParseNBStatResponse(buf[:n])
}
