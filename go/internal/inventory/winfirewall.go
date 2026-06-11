// SPDX-License-Identifier: GPL-2.0-only

package inventory

// winFirewallProfiles maps the three Windows firewall profiles to their registry
// subkey, in the sorted order the upstream Win32/Firewall.pm emits them
// (domain, public, standard).
var winFirewallProfiles = []struct{ key, subKey string }{
	{"domain", "DomainProfile"},
	{"public", "PublicProfile"},
	{"standard", "StandardProfile"},
}

// buildWinFirewall maps the per-profile EnableFirewall flag to FIREWALL entries,
// mirroring Win32/Firewall.pm _getFirewallProfiles: each profile yields a STATUS
// ("on"/"off") and the PROFILE subkey name. The per-connection association
// (CONNECTIONS/IPADDRESS from the NetworkList registry + interfaces) is follow-on.
func buildWinFirewall(enabled map[string]bool) []map[string]any {
	out := make([]map[string]any, 0, len(winFirewallProfiles))
	for _, p := range winFirewallProfiles {
		status := "off"
		if enabled[p.key] {
			status = "on"
		}
		out = append(out, map[string]any{"STATUS": status, "PROFILE": p.subKey})
	}
	return out
}
