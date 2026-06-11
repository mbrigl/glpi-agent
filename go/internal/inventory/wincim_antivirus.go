// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "fmt"

// winAntivirusProperties are the AntiVirusProduct properties read from the
// root/SecurityCenter(2) WMI namespace (Win32/AntiVirus.pm).
var winAntivirusProperties = []string{
	"companyName", "displayName", "instanceGuid", "onAccessScanningEnabled",
	"productUptoDate", "versionNumber", "productState",
}

// buildWinAntivirus maps AntiVirusProduct objects (root/SecurityCenter2) to the
// ANTIVIRUS section, mirroring Win32/AntiVirus.pm: COMPANY/NAME/GUID/VERSION and
// the ENABLED/UPTODATE flags. When productState is present its hex encoding wins
// over the onAccessScanningEnabled/productUptoDate booleans (see
// http://neophob.com/2010/03/wmi-query-windows-securitycenter2/). Entries are
// deduplicated by NAME+VERSION across both namespaces. The vendor-specific
// version/expiration enrichment (McAfee, Kaspersky, Defender, ...) is registry/
// command based and is follow-on.
func buildWinAntivirus(objects []map[string]any) []map[string]any {
	var out []map[string]any
	seen := map[string]bool{}
	for _, o := range objects {
		name := cimString(o, "displayName")
		if name == "" {
			continue
		}

		av := map[string]any{
			"NAME":     name,
			"ENABLED":  boolToInt(cimBool(o, "onAccessScanningEnabled")),
			"UPTODATE": boolToInt(cimBool(o, "productUptoDate")),
		}
		setIf(av, "COMPANY", cimString(o, "companyName"))
		setIf(av, "GUID", cimString(o, "instanceGuid"))
		setIf(av, "VERSION", cimString(o, "versionNumber"))

		if state := cimInt(o, "productState"); state != 0 {
			if enabled, uptodate, ok := decodeProductState(state); ok {
				av["ENABLED"] = enabled
				av["UPTODATE"] = uptodate
			}
		}

		key := name + "\x00" + cimString(o, "versionNumber")
		if seen[key] {
			continue
		}
		seen[key] = true
		out = append(out, av)
	}
	return out
}

// decodeProductState decodes a SecurityCenter2 productState integer the way
// Win32/AntiVirus.pm does: dec2hex then match the trailing two byte-pairs of the
// hex string ((.{2})(.{2})$). ENABLED is 1 when the first pair starts with "1";
// UPTODATE is 1 when the second pair is "00". ok is false when the hex string is
// too short to carry both pairs (mirroring the undef regex capture).
func decodeProductState(state int) (enabled, uptodate int, ok bool) {
	hex := fmt.Sprintf("0x%x", state)
	if len(hex) < 4 {
		return 0, 0, false
	}
	last4 := hex[len(hex)-4:]
	first, second := last4[:2], last4[2:]
	enabled = boolToInt(first[0] == '1')
	uptodate = boolToInt(second == "00")
	return enabled, uptodate, true
}

// winEnvironmentProperties are the Win32_Environment properties for ENVS.
var winEnvironmentProperties = []string{"SystemVariable", "Name", "VariableValue"}

// buildWinEnvironment maps Win32_Environment to the ENVS section, mirroring
// Win32/Environment.pm: only the system variables (SystemVariable true) are
// kept, as KEY/VAL pairs.
func buildWinEnvironment(objects []map[string]any) []map[string]any {
	var out []map[string]any
	for _, o := range objects {
		if sv := cimString(o, "SystemVariable"); sv != "1" && sv != "true" {
			continue
		}
		name := cimString(o, "Name")
		if name == "" {
			continue
		}
		out = append(out, map[string]any{"KEY": name, "VAL": cimString(o, "VariableValue")})
	}
	return out
}
