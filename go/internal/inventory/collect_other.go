// SPDX-License-Identifier: GPL-2.0-only

//go:build !linux

package inventory

// Collect gathers the local inventory on non-Linux platforms. Only the
// host-identifying basics are collected for now; the Windows and macOS category
// collectors are later Phase 6 work (internal/inventory/{windows,macos}).
func Collect() Sections {
	s := Sections{}
	s.mergeHardware(map[string]any{"NAME": hostname()})
	return s
}
