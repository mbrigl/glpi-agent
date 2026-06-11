// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import (
	"regexp"

	"golang.org/x/sys/windows/registry"
)

const winEnumPath = `SYSTEM\CurrentControlSet\Enum`

// winMonitorIDSuffixRE strips the trailing "_<n>" from a WMIMonitorConnectionParams
// InstanceName so it matches the registry Enum subkey / Win32_DesktopMonitor id.
var winMonitorIDSuffixRE = regexp.MustCompile(`_\d+$`)

// collectWinMonitors gathers the MONITORS section from Win32_DesktopMonitor, the
// root/wmi connection-param ids and the registry EDID blocks, mirroring
// Generic/Screen.pm _getScreensFromWindows.
func collectWinMonitors() []map[string]any {
	desktop, _ := powershellCIM("Win32_DesktopMonitor", winDesktopMonitorProperties)
	extraIDs := winMonitorConnectionIDs()

	ids := append([]string{}, extraIDs...)
	for _, o := range desktop {
		if p := cimString(o, "PNPDeviceID"); p != "" {
			ids = append(ids, p)
		}
	}

	edid := map[string][]byte{}
	for _, id := range ids {
		if _, ok := edid[id]; ok {
			continue
		}
		if b := readMonitorEDID(id); len(b) > 0 {
			edid[id] = b
		}
	}

	return buildWinMonitors(desktop, extraIDs, edid)
}

// winMonitorConnectionIDs returns the active screen ids from the root/wmi
// WMIMonitorConnectionParams class (trailing "_<n>" stripped).
func winMonitorConnectionIDs() []string {
	objs, err := powershellCIMNamespace("root/wmi", "WMIMonitorConnectionParams", []string{"Active", "InstanceName"})
	if err != nil {
		return nil
	}
	var ids []string
	for _, o := range objs {
		if !cimBool(o, "Active") {
			continue
		}
		name := cimString(o, "InstanceName")
		if name == "" {
			continue
		}
		ids = append(ids, winMonitorIDSuffixRE.ReplaceAllString(name, ""))
	}
	return ids
}

// readMonitorEDID reads the EDID_OVERRIDE (preferred) or EDID binary value from
// the screen's Device Parameters registry key.
func readMonitorEDID(id string) []byte {
	key, err := registry.OpenKey(registry.LOCAL_MACHINE, winEnumPath+`\`+id+`\Device Parameters`, registry.QUERY_VALUE)
	if err != nil {
		return nil
	}
	defer key.Close()
	for _, name := range []string{"EDID_OVERRIDE", "EDID"} {
		if b, _, err := key.GetBinaryValue(name); err == nil && len(b) > 0 {
			return b
		}
	}
	return nil
}
