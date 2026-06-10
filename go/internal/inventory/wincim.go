// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bytes"
	"encoding/json"
	"regexp"
	"strings"
)

// Windows inventory is collected by running PowerShell
// `Get-CimInstance <Win32_class> | Select <props> | ConvertTo-Json` and mapping
// the CIM objects to the GLPI sections, mirroring the WMI queries of the
// upstream Task/Inventory/Win32/* modules. The query runner lives in the
// //go:build windows collector; the value mapping here is pure and unit-tested
// on any platform against CIM-JSON fixtures.

// decodeCIMJSON decodes the output of `ConvertTo-Json` into a slice of objects.
// ConvertTo-Json emits a bare object for a single result and an array for many,
// so both are normalised to a slice.
func decodeCIMJSON(data []byte) ([]map[string]any, error) {
	data = bytes.TrimSpace(data)
	if len(data) == 0 {
		return nil, nil
	}
	if data[0] == '[' {
		var arr []map[string]any
		if err := json.Unmarshal(data, &arr); err != nil {
			return nil, err
		}
		return arr, nil
	}
	var obj map[string]any
	if err := json.Unmarshal(data, &obj); err != nil {
		return nil, err
	}
	return []map[string]any{obj}, nil
}

// cimString returns a CIM property as a trimmed string ("" when absent or null).
func cimString(obj map[string]any, key string) string {
	v, ok := obj[key]
	if !ok || v == nil {
		return ""
	}
	switch s := v.(type) {
	case string:
		return strings.TrimSpace(s)
	default:
		return strings.TrimSpace(jsonScalar(v))
	}
}

// jsonScalar renders a non-string JSON scalar (numbers/bools) as text.
func jsonScalar(v any) string {
	b, err := json.Marshal(v)
	if err != nil {
		return ""
	}
	return strings.Trim(string(b), `"`)
}

var wmiDateTimeRE = regexp.MustCompile(`^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})\.\d{6}.\d{3}$`)
var isoDateTimeRE = regexp.MustCompile(`^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$`)

// wmiDateTime formats a WMI CIM_DATETIME ("YYYYMMDDHHMMSS.ffffff±UUU") as
// "YYYY-MM-DD HH:MM:SS", mirroring Tools/Win32::getFormatedWMIDateTime. An
// already-ISO value is returned unchanged; anything else yields "".
func wmiDateTime(s string) string {
	s = strings.TrimSpace(s)
	if isoDateTimeRE.MatchString(s) {
		return s
	}
	m := wmiDateTimeRE.FindStringSubmatch(s)
	if m == nil {
		return ""
	}
	return m[1] + "-" + m[2] + "-" + m[3] + " " + m[4] + ":" + m[5] + ":" + m[6]
}

// winOSProperties are the Win32_OperatingSystem properties the OS section needs.
var winOSProperties = []string{
	"Caption", "Version", "CSDVersion", "LastBootUpTime", "InstallDate",
	"BuildNumber", "OSArchitecture",
}

// buildWinOS maps a Win32_OperatingSystem CIM object to the OPERATINGSYSTEM
// section, mirroring Win32/OS.pm. The registry refinements (UBR, DisplayVersion,
// InstallDate fallback) are follow-on.
func buildWinOS(os map[string]any) map[string]any {
	out := map[string]any{
		"NAME": "Windows",
		"ARCH": winArch(cimString(os, "OSArchitecture")),
	}
	setIf(out, "KERNEL_VERSION", cimString(os, "Version"))
	setIf(out, "FULL_NAME", cimString(os, "Caption"))
	setIf(out, "BOOT_TIME", wmiDateTime(cimString(os, "LastBootUpTime")))
	setIf(out, "INSTALL_DATE", wmiDateTime(cimString(os, "InstallDate")))
	setIf(out, "SERVICE_PACK", cimString(os, "CSDVersion"))
	return out
}

// winArch maps Win32_OperatingSystem.OSArchitecture to the GLPI ARCH value
// (Win32/OS.pm: ARM -> Arm64, 64-bit, else 32-bit).
func winArch(osArch string) string {
	switch {
	case regexp.MustCompile(`(?i)arm`).MatchString(osArch):
		return "Arm64"
	case strings.Contains(osArch, "64"):
		return "64-bit"
	default:
		return "32-bit"
	}
}
