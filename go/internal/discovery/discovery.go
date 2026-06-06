// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"encoding/binary"
	"fmt"
	"net"
	"strings"
)

// Device is a discovered device, keyed by the canonical UPPERCASE DEVICE field
// names from SNMP/Device.pm.
type Device map[string]any

// BuildDevice assembles a DEVICE from the generic OID values, mirroring the
// base-variable mapping in SNMP/Device.pm. It returns nil when the host did not
// answer with a sysDescr (i.e. it is not SNMP-capable), matching NetDiscovery's
// "only report devices that respond to SNMP".
func BuildDevice(ip string, values map[string]string) Device {
	descr := strings.TrimSpace(values[oidSysDescr])
	if descr == "" {
		return nil
	}
	device := Device{
		"IP":          ip,
		"DESCRIPTION": descr,
	}
	set := func(field, oid string) {
		if v := strings.TrimSpace(values[oid]); v != "" {
			device[field] = v
		}
	}
	set("SNMPHOSTNAME", oidSysName)
	set("LOCATION", oidSysLocation)
	set("CONTACT", oidSysContact)
	set("UPTIME", oidSysUpTime)
	// sysObjectID is kept for the (follow-on) TYPE/MANUFACTURER/MODEL
	// classification driven by the vendor MibSupport tail.
	set("SYSOBJECTID", oidSysObjectID)
	return device
}

// Probe queries the generic OIDs of one host and builds its DEVICE entry, or nil
// if the host is not SNMP-capable.
func Probe(ip string, getter SNMPGetter) (Device, error) {
	values, err := getter.Get(genericOIDs)
	if err != nil {
		return nil, err
	}
	return BuildDevice(ip, values), nil
}

// Dialer opens an SNMPGetter for a host. It is injected so scans can be tested
// without real network access.
type Dialer func(host string) (SNMPGetter, error)

// Scan probes every address in the given ranges and returns the SNMP-capable
// devices found. Addresses that do not answer are skipped. The scan is
// sequential; concurrent workers (the --threads option) are follow-on.
func Scan(ranges []string, dial Dialer) ([]Device, error) {
	var devices []Device
	for _, spec := range ranges {
		ips, err := ParseRange(spec)
		if err != nil {
			return nil, err
		}
		for _, ip := range ips {
			getter, err := dial(ip)
			if err != nil {
				continue // host unreachable / no SNMP
			}
			device, err := Probe(ip, getter)
			_ = getter.Close()
			if err != nil || device == nil {
				continue
			}
			devices = append(devices, device)
		}
	}
	return devices, nil
}

// ParseRange expands an IPv4 spec into addresses. It accepts a single IP, a CIDR
// (a.b.c.d/n) or an inclusive range (a.b.c.d-e.f.g.h), mirroring the IP-range
// inputs NetDiscovery accepts.
func ParseRange(spec string) ([]string, error) {
	spec = strings.TrimSpace(spec)

	if strings.Contains(spec, "/") {
		return cidrHosts(spec)
	}
	if strings.Contains(spec, "-") {
		parts := strings.SplitN(spec, "-", 2)
		return ipRange(strings.TrimSpace(parts[0]), strings.TrimSpace(parts[1]))
	}
	ip := net.ParseIP(spec)
	if ip == nil || ip.To4() == nil {
		return nil, fmt.Errorf("invalid IPv4 address %q", spec)
	}
	return []string{ip.String()}, nil
}

func cidrHosts(spec string) ([]string, error) {
	_, ipnet, err := net.ParseCIDR(spec)
	if err != nil {
		return nil, err
	}
	if ipnet.IP.To4() == nil {
		return nil, fmt.Errorf("only IPv4 CIDR is supported: %q", spec)
	}
	start := binary.BigEndian.Uint32(ipnet.IP.To4())
	mask := binary.BigEndian.Uint32(net.IP(ipnet.Mask).To4())
	network := start & mask
	broadcast := network | ^mask

	var ips []string
	for v := network; v <= broadcast; v++ {
		// Skip the network and broadcast addresses for a normal subnet.
		if (broadcast-network) >= 2 && (v == network || v == broadcast) {
			continue
		}
		ips = append(ips, uint32ToIP(v))
		if v == ^uint32(0) {
			break // guard against overflow at 255.255.255.255
		}
	}
	return ips, nil
}

func ipRange(startStr, endStr string) ([]string, error) {
	start := net.ParseIP(startStr)
	end := net.ParseIP(endStr)
	if start == nil || start.To4() == nil || end == nil || end.To4() == nil {
		return nil, fmt.Errorf("invalid IPv4 range %q-%q", startStr, endStr)
	}
	s := binary.BigEndian.Uint32(start.To4())
	e := binary.BigEndian.Uint32(end.To4())
	if s > e {
		return nil, fmt.Errorf("range start %s is after end %s", startStr, endStr)
	}
	var ips []string
	for v := s; v <= e; v++ {
		ips = append(ips, uint32ToIP(v))
		if v == ^uint32(0) {
			break
		}
	}
	return ips, nil
}

func uint32ToIP(v uint32) string {
	var b [4]byte
	binary.BigEndian.PutUint32(b[:], v)
	return net.IP(b[:]).String()
}
