// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package collect

// DefaultModules returns the collection modules available on Windows: the
// cross-platform findFile plus the Windows-only getFromRegistry / getFromWMI.
func DefaultModules() []Module {
	return []Module{FileCollector{}, RegistryCollector{}, WMICollector{}}
}
