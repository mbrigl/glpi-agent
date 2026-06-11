// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import "golang.org/x/sys/windows/registry"

const winFirewallPolicyPath = `SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy`

// collectWinFirewall reads the per-profile EnableFirewall DWORD from the registry
// and maps it to the FIREWALL section (Win32/Firewall.pm _getFirewallProfiles).
func collectWinFirewall() []map[string]any {
	enabled := map[string]bool{}
	for _, p := range winFirewallProfiles {
		key, err := registry.OpenKey(registry.LOCAL_MACHINE, winFirewallPolicyPath+`\`+p.subKey, registry.QUERY_VALUE)
		if err != nil {
			continue
		}
		v, _, err := key.GetIntegerValue("EnableFirewall")
		key.Close()
		if err == nil && v != 0 {
			enabled[p.key] = true
		}
	}
	return buildWinFirewall(enabled)
}
