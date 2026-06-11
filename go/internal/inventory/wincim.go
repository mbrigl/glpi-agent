// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bytes"
	"encoding/json"
	"regexp"
	"strconv"
	"strings"
	"time"
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

var (
	// Raw CIM_DATETIME: "YYYYMMDDHHMMSS.ffffff±UUU".
	wmiDateTimeRE = regexp.MustCompile(`^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})\.\d{6}.\d{3}$`)
	// Canonical output (and pass-through): "YYYY-MM-DD HH:MM:SS".
	isoDateTimeRE = regexp.MustCompile(`^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$`)
	// ISO-8601 with a "T" separator (PowerShell ConvertTo-Json of a [DateTime]),
	// e.g. "2024-01-15T08:30:00.5000000+01:00" — fractional seconds and the
	// timezone suffix are ignored, the local wall-clock components are kept.
	isoTDateTimeRE = regexp.MustCompile(`^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})`)
	// Microsoft JSON date ("\/Date(<ms-since-epoch>[±HHMM])\/") emitted by the
	// JavaScriptSerializer path of Windows PowerShell 5.1.
	msJSONDateRE = regexp.MustCompile(`^/Date\((-?\d+)(?:([+-]\d{2})(\d{2}))?\)/$`)
)

// wmiDateTime normalises a CIM datetime to the canonical "YYYY-MM-DD HH:MM:SS"
// (local wall-clock), mirroring Tools/Win32::getFormatedWMIDateTime. It accepts
// the raw CIM_DATETIME format, the already-canonical form, and — because Windows
// inventory flows through PowerShell `ConvertTo-Json`, which serialises a
// [DateTime] rather than the raw string — the ISO-8601-with-"T" and the
// Microsoft "/Date(ms)/" serialisations. The timezone is ignored for the string
// forms (as upstream does); anything unrecognised yields "".
func wmiDateTime(s string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		return ""
	}
	if isoDateTimeRE.MatchString(s) {
		return s
	}
	if m := wmiDateTimeRE.FindStringSubmatch(s); m != nil {
		return m[1] + "-" + m[2] + "-" + m[3] + " " + m[4] + ":" + m[5] + ":" + m[6]
	}
	if m := isoTDateTimeRE.FindStringSubmatch(s); m != nil {
		return m[1] + "-" + m[2] + "-" + m[3] + " " + m[4] + ":" + m[5] + ":" + m[6]
	}
	if m := msJSONDateRE.FindStringSubmatch(s); m != nil {
		return msJSONDateToCanonical(m[1], m[2], m[3])
	}
	return ""
}

// msJSONDateToCanonical converts a Microsoft "/Date(ms±HHMM)/" capture to the
// canonical local wall-clock string. With an explicit offset the wall clock is
// recovered from it; without one the value is UTC and rendered in the host's
// local zone (the agent runs on the inventoried machine, so this matches the
// machine's local time).
func msJSONDateToCanonical(msStr, offHour, offMin string) string {
	ms, err := strconv.ParseInt(msStr, 10, 64)
	if err != nil {
		return ""
	}
	t := time.UnixMilli(ms)
	if offHour != "" {
		h, _ := strconv.Atoi(offHour)
		m, _ := strconv.Atoi(offMin)
		sign := 1
		if h < 0 {
			sign, h = -1, -h
		}
		t = t.In(time.FixedZone("", sign*(h*3600+m*60)))
	} else {
		t = t.Local()
	}
	return t.Format("2006-01-02 15:04:05")
}

// winOSProperties are the Win32_OperatingSystem properties the operatingsystem
// and hardware sections need (fetched in one query).
var winOSProperties = []string{
	"Caption", "Version", "CSDVersion", "LastBootUpTime", "InstallDate",
	"BuildNumber", "OSArchitecture",
	"OSLanguage", "SerialNumber", "Organization", "RegisteredUser", "TotalSwapSpaceSize",
	"SystemDrive",
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
