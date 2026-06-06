// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/binary"
	"net"
)

// NetIface is the OS-independent view of a network interface used to build the
// NETWORKS section. The build-tagged collector fills it from net.Interfaces()
// and sysfs.
type NetIface struct {
	Name    string
	MAC     string
	Up      bool
	Virtual bool
	Speed   int // Mb/s, 0 if unknown
	Driver  string
	Addrs   []NetAddr
}

// NetAddr is one IPv4 address with its dotted netmask.
type NetAddr struct {
	IP   string
	Mask string
}

// BuildNetworks assembles the NETWORKS entries, mirroring the field set of
// Linux/Networks.pm: DESCRIPTION, MACADDR, STATUS, VIRTUALDEV, SPEED, DRIVER,
// and (from the first IPv4 address) IPADDRESS/IPMASK/IPSUBNET.
func BuildNetworks(ifaces []NetIface) []map[string]any {
	var out []map[string]any
	for _, ni := range ifaces {
		entry := map[string]any{
			"DESCRIPTION": ni.Name,
			"VIRTUALDEV":  boolToInt(ni.Virtual),
		}
		setIf(entry, "MACADDR", ni.MAC)
		setIf(entry, "DRIVER", ni.Driver)
		if ni.Up {
			entry["STATUS"] = "Up"
		} else {
			entry["STATUS"] = "Down"
		}
		if ni.Speed > 0 {
			entry["SPEED"] = ni.Speed
		}
		if len(ni.Addrs) > 0 {
			a := ni.Addrs[0]
			setIf(entry, "IPADDRESS", a.IP)
			setIf(entry, "IPMASK", a.Mask)
			if subnet := subnetAddress(a.IP, a.Mask); subnet != "" {
				entry["IPSUBNET"] = subnet
			}
		}
		out = append(out, entry)
	}
	return out
}

// subnetAddress returns the network address for an IPv4 address and dotted mask,
// mirroring getSubnetAddress.
func subnetAddress(ipStr, maskStr string) string {
	ip := net.ParseIP(ipStr).To4()
	mask := net.ParseIP(maskStr).To4()
	if ip == nil || mask == nil {
		return ""
	}
	var b [4]byte
	binary.BigEndian.PutUint32(b[:], binary.BigEndian.Uint32(ip)&binary.BigEndian.Uint32(mask))
	return net.IP(b[:]).String()
}

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}
