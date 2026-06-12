// SPDX-License-Identifier: GPL-2.0-only

//go:build !windows

package collect

// DefaultModules returns the collection modules available on this platform. The
// findFile module is cross-platform; getFromRegistry/getFromWMI are Windows-only
// (see modules_windows.go). The runCommand module is disabled upstream.
func DefaultModules() []Module {
	return []Module{FileCollector{}}
}
