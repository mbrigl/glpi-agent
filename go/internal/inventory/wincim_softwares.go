// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strconv"
	"strings"
)

// winSoftwareEntry is one Uninstall registry subkey: its name and its string
// values (DWORD values are rendered as decimal strings by the reader).
type winSoftwareEntry struct {
	Key    string
	Values map[string]string
}

// winSoftwareValueMap maps the GLPI SOFTWARES field to its Uninstall value name
// (Win32/Softwares.pm %mapping).
var winSoftwareValueMap = map[string]string{
	"COMMENTS":         "Comments",
	"HELPLINK":         "HelpLink",
	"RELEASE_TYPE":     "ReleaseType",
	"PUBLISHER":        "Publisher",
	"URL_INFO_ABOUT":   "URLInfoAbout",
	"UNINSTALL_STRING": "UninstallString",
}

var winVersionCtrlRE = regexp.MustCompile(`[\x00-\x1f].*`)

// buildWinSoftwares maps Uninstall registry subkeys to SOFTWARES entries,
// mirroring Win32/Softwares.pm: NAME from DisplayName (default the subkey),
// VERSION (control chars stripped), the value mapping, INSTALLDATE, the
// hex DWORD fields and SYSTEM_CATEGORY. Subkeys with a single value are skipped
// and entries are deduplicated by NAME+ARCH+VERSION. arch is "x86_64" or "i586".
func buildWinSoftwares(entries []winSoftwareEntry, arch string) []map[string]any {
	var out []map[string]any
	seen := map[string]bool{}

	for _, e := range entries {
		if len(e.Values) <= 1 {
			continue // CntValues > 1
		}
		name := winFirstNonEmpty(e.Values["DisplayName"], e.Key)
		if name == "" {
			continue
		}

		sw := map[string]any{
			"FROM": "registry",
			"NAME": name,
			"ARCH": arch,
			"GUID": e.Key,
		}
		for field, value := range winSoftwareValueMap {
			setIf(sw, field, e.Values[value])
		}
		if v := strings.TrimSpace(winVersionCtrlRE.ReplaceAllString(e.Values["DisplayVersion"], "")); v != "" {
			sw["VERSION"] = v
		}
		if d := winSoftwareDate(e.Values["InstallDate"]); d != "" {
			sw["INSTALLDATE"] = d
		}
		if n, ok := hex2dec(e.Values["MinorVersion"]); ok {
			sw["VERSION_MINOR"] = n
		}
		if n, ok := hex2dec(e.Values["MajorVersion"]); ok {
			sw["VERSION_MAJOR"] = n
		}
		if n, ok := hex2dec(e.Values["NoRemove"]); ok {
			sw["NO_REMOVE"] = n
		}
		if n, ok := hex2dec(e.Values["SystemComponent"]); ok && n != 0 {
			sw["SYSTEM_CATEGORY"] = "system_component"
		} else {
			sw["SYSTEM_CATEGORY"] = "application"
		}

		version, _ := sw["VERSION"].(string)
		key := name + "\x00" + arch + "\x00" + version
		if seen[key] {
			continue
		}
		seen[key] = true
		out = append(out, sw)
	}
	return out
}

// hex2dec parses a registry DWORD, accepting a "0x" hex form or a decimal
// string. ok is false when the value is empty or unparsable.
func hex2dec(s string) (int, bool) {
	s = strings.TrimSpace(s)
	if s == "" {
		return 0, false
	}
	if strings.HasPrefix(s, "0x") || strings.HasPrefix(s, "0X") {
		n, err := strconv.ParseInt(s[2:], 16, 64)
		return int(n), err == nil
	}
	n, err := strconv.Atoi(s)
	return n, err == nil
}

var (
	winDate8RE = regexp.MustCompile(`^(\d{4})(\d{2})(\d{2})$`)
	winDate7RE = regexp.MustCompile(`^(\d{4})(\d{1})(\d{2})$`)
)

// winSoftwareDate formats an Uninstall InstallDate ("YYYYMMDD") as "DD/MM/YYYY",
// mirroring Win32/Softwares.pm::_dateFormat; other input is left unchanged.
func winSoftwareDate(s string) string {
	s = strings.TrimSpace(s)
	if m := winDate8RE.FindStringSubmatch(s); m != nil {
		return m[3] + "/" + m[2] + "/" + m[1]
	}
	if m := winDate7RE.FindStringSubmatch(s); m != nil {
		return m[3] + "/0" + m[2] + "/" + m[1]
	}
	return s
}
