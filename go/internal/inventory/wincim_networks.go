// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "strings"

var (
	winNetAdapterProperties = []string{
		"Index", "InterfaceIndex", "PNPDeviceID", "Speed", "PhysicalAdapter",
		"GUID", "Description", "AdapterType",
	}
	winNetConfigProperties = []string{
		"Index", "InterfaceIndex", "Description", "IPEnabled", "DHCPServer",
		"MACAddress", "MTU", "DefaultIPGateway", "DNSServerSearchOrder",
		"IPAddress", "IPSubnet", "SettingID",
	}
)

// buildWinNetworks joins Win32_NetworkAdapter with Win32_NetworkAdapterConfiguration
// (by Index) and emits one NETWORKS entry per IP address, mirroring
// Tools/Win32/NetAdapter.pm. The VPN (registry) interfaces, the DNS/gateway
// document-level aggregation and the MSFT_NetAdapter InterfaceType enum are
// follow-on.
func buildWinNetworks(adapters, configs []map[string]any) []map[string]any {
	cfgByIndex := map[int]map[string]any{}
	for _, c := range configs {
		cfgByIndex[cimInt(c, "Index")] = c
	}

	var nets []map[string]any
	for _, a := range adapters {
		cfg := cfgByIndex[cimInt(a, "Index")]
		if cfg == nil {
			continue
		}
		if cimString(cfg, "MACAddress") == "" {
			continue // no MAC and not a vpn -> skip
		}

		base := winBaseInterface(a, cfg)
		addrs := winZipAddresses(cfg)
		if len(addrs) == 0 {
			nets = append(nets, base)
			continue
		}
		for _, ad := range addrs {
			iface := winCloneMap(base)
			ip, subnet := ad[0], ad[1]
			if strings.Contains(ip, ":") {
				iface["IPADDRESS6"] = stripZoneID(ip)
			} else {
				iface["IPADDRESS"] = ip
				if subnet != "" {
					iface["IPMASK"] = subnet
					if sub := subnetAddress(ip, subnet); sub != "" {
						iface["IPSUBNET"] = sub
					}
				}
			}
			nets = append(nets, iface)
		}
	}
	return nets
}

// winBaseInterface builds the scalar NETWORKS fields shared by every address of
// an adapter.
func winBaseInterface(a, cfg map[string]any) map[string]any {
	b := map[string]any{}
	setIf(b, "DESCRIPTION", winFirstNonEmpty(cimString(cfg, "Description"), cimString(a, "Description")))
	setIf(b, "MACADDR", cimString(cfg, "MACAddress"))
	if cimBool(cfg, "IPEnabled") {
		b["STATUS"] = "Up"
	} else {
		b["STATUS"] = "Down"
	}
	if mtu := cimInt(cfg, "MTU"); mtu > 0 {
		b["MTU"] = mtu
	}
	setIf(b, "IPDHCP", cimString(cfg, "DHCPServer"))
	setIf(b, "IPGATEWAY", cimFirstOfArray(cfg, "DefaultIPGateway"))
	setIf(b, "GUID", cimString(cfg, "SettingID"))
	if sp := cimInt64(a, "Speed"); sp > 0 {
		b["SPEED"] = int(sp / 1_000_000)
	}
	b["VIRTUALDEV"] = boolToInt(winIsVirtual(a))
	return b
}

// winIsVirtual reports whether an adapter is virtual (PNPDeviceID under ROOT, or
// not a physical/hardware adapter), mirroring NetAdapter::_isVirtual.
func winIsVirtual(a map[string]any) bool {
	if strings.HasPrefix(cimString(a, "PNPDeviceID"), "ROOT") {
		return true
	}
	// Physical when PhysicalAdapter (or the MSFT HardwareInterface) is set.
	return !(cimBool(a, "PhysicalAdapter") || cimBool(a, "HardwareInterface"))
}

// winZipAddresses pairs the parallel IPAddress / IPSubnet arrays of a
// configuration into [ip, subnet] tuples.
func winZipAddresses(cfg map[string]any) [][2]string {
	ips := cimStringArray(cfg, "IPAddress")
	subnets := cimStringArray(cfg, "IPSubnet")
	var out [][2]string
	for i, ip := range ips {
		subnet := ""
		if i < len(subnets) {
			subnet = subnets[i]
		}
		out = append(out, [2]string{ip, subnet})
	}
	return out
}

// cimStringArray returns a CIM array property as a string slice (a single scalar
// is returned as a one-element slice).
func cimStringArray(obj map[string]any, key string) []string {
	v, ok := obj[key]
	if !ok || v == nil {
		return nil
	}
	switch t := v.(type) {
	case []any:
		out := make([]string, 0, len(t))
		for _, e := range t {
			if s, ok := e.(string); ok {
				out = append(out, s)
			} else {
				out = append(out, jsonScalar(e))
			}
		}
		return out
	case string:
		return []string{t}
	default:
		return []string{jsonScalar(v)}
	}
}

// cimFirstOfArray returns the first element of an array CIM property (or the
// scalar itself).
func cimFirstOfArray(obj map[string]any, key string) string {
	if a := cimStringArray(obj, key); len(a) > 0 {
		return a[0]
	}
	return ""
}

// stripZoneID drops a "%<zone>" suffix from an IPv6 address.
func stripZoneID(ip string) string {
	if i := strings.IndexByte(ip, '%'); i >= 0 {
		return ip[:i]
	}
	return ip
}

// winCloneMap shallow-copies a map so per-address interfaces can extend it.
func winCloneMap(m map[string]any) map[string]any {
	out := make(map[string]any, len(m)+3)
	for k, v := range m {
		out[k] = v
	}
	return out
}
