// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import (
	"strconv"

	"golang.org/x/sys/windows/registry"
)

// uninstallPath is the registry path holding the installed-software entries.
const uninstallPath = `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`

// winSoftwareStringValues / winSoftwareDWORDValues are the values read from each
// Uninstall subkey (Win32/Softwares.pm).
var (
	winSoftwareStringValues = []string{
		"DisplayName", "DisplayVersion", "Publisher", "Comments", "HelpLink",
		"ReleaseType", "URLInfoAbout", "UninstallString", "InstallDate",
	}
	winSoftwareDWORDValues = []string{"MinorVersion", "MajorVersion", "NoRemove", "SystemComponent"}
)

// collectWinSoftwares reads the installed software from the 64-bit and 32-bit
// Uninstall registry views and maps them to SOFTWARES entries.
func collectWinSoftwares() []map[string]any {
	var softwares []map[string]any
	views := []struct {
		access uint32
		arch   string
	}{
		{registry.READ | registry.WOW64_64KEY, "x86_64"},
		{registry.READ | registry.WOW64_32KEY, "i586"},
	}
	for _, v := range views {
		entries := readUninstallEntries(registry.LOCAL_MACHINE, v.access)
		softwares = append(softwares, buildWinSoftwares(entries, v.arch)...)
	}
	return softwares
}

// readUninstallEntries enumerates the Uninstall subkeys under the given hive/view
// and reads their values.
func readUninstallEntries(hive registry.Key, access uint32) []winSoftwareEntry {
	root, err := registry.OpenKey(hive, uninstallPath, access|registry.ENUMERATE_SUB_KEYS)
	if err != nil {
		return nil
	}
	defer root.Close()

	names, err := root.ReadSubKeyNames(-1)
	if err != nil {
		return nil
	}

	var entries []winSoftwareEntry
	for _, name := range names {
		sub, err := registry.OpenKey(hive, uninstallPath+`\`+name, access)
		if err != nil {
			continue
		}
		values := map[string]string{}
		for _, vn := range winSoftwareStringValues {
			if s, _, err := sub.GetStringValue(vn); err == nil {
				values[vn] = s
			}
		}
		for _, vn := range winSoftwareDWORDValues {
			if n, _, err := sub.GetIntegerValue(vn); err == nil {
				values[vn] = strconv.FormatUint(n, 10)
			}
		}
		sub.Close()
		entries = append(entries, winSoftwareEntry{Key: name, Values: values})
	}
	return entries
}
