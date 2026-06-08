// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"encoding/binary"
	"fmt"
	"net"
	"strings"
	"sync"
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

// HostProbe returns the non-SNMP discovery fields (arp/netbios) for an address,
// or an empty HostInfo when nothing is found. It may be nil.
type HostProbe func(ip string) HostInfo

// Scan probes every address in the given ranges with up to `threads` concurrent
// workers and returns the discovered devices. A device is reported when it
// answers SNMP or when hostProbe yields an identifying field (MAC / DNS /
// NetBIOS name), mirroring the multi-method _scanAddress of NetDiscovery.pm.
// threads <= 1 scans sequentially. Mirrors the Parallel::ForkManager worker pool.
func Scan(ranges []string, dial Dialer, threads int, hostProbe HostProbe) ([]Device, error) {
	var ips []string
	for _, spec := range ranges {
		expanded, err := ParseRange(spec)
		if err != nil {
			return nil, err
		}
		ips = append(ips, expanded...)
	}
	if threads < 1 {
		threads = 1
	}
	if threads > len(ips) {
		threads = len(ips)
	}
	if threads <= 1 {
		var devices []Device
		for _, ip := range ips {
			if d := probeOne(ip, dial, hostProbe); d != nil {
				devices = append(devices, d)
			}
		}
		return devices, nil
	}

	jobs := make(chan string)
	results := make(chan Device)
	var wg sync.WaitGroup
	for i := 0; i < threads; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for ip := range jobs {
				if d := probeOne(ip, dial, hostProbe); d != nil {
					results <- d
				}
			}
		}()
	}
	go func() {
		for _, ip := range ips {
			jobs <- ip
		}
		close(jobs)
		wg.Wait()
		close(results)
	}()

	var devices []Device
	for d := range results {
		devices = append(devices, d)
	}
	return devices, nil
}

// probeOne probes one address over SNMP and merges any non-SNMP discovery
// fields. It returns nil when neither SNMP nor the host probe identifies a
// device, mirroring the "keep if MAC/DNS/NetBIOS/IP/SNMP" rule of _scanAddress.
func probeOne(ip string, dial Dialer, hostProbe HostProbe) Device {
	var device Device
	if getter, err := dial(ip); err == nil {
		device, _ = Probe(ip, getter)
		getter.Close()
	}

	var host HostInfo
	if hostProbe != nil {
		host = hostProbe(ip)
	}

	if device == nil {
		if !host.Any() {
			return nil
		}
		device = Device{"IP": ip}
	}
	// Fill fields the SNMP probe did not provide.
	setHostField(device, "MAC", host.MAC)
	setHostField(device, "DNSHOSTNAME", host.DNSHostname)
	setHostField(device, "NETBIOSNAME", host.NetbiosName)
	setHostField(device, "WORKGROUP", host.Workgroup)
	return device
}

func setHostField(d Device, key, val string) {
	if val == "" {
		return
	}
	if _, ok := d[key]; !ok {
		d[key] = val
	}
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
