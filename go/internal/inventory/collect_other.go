// SPDX-License-Identifier: GPL-2.0-only

//go:build !linux && !windows

package inventory

// Collect gathers the local inventory on the platforms without a dedicated
// collector yet (macOS, *BSD, …). Only the host-identifying basics are
// collected; full collectors are later work.
func Collect() Sections {
	s := Sections{}
	s.mergeHardware(map[string]any{"NAME": hostname()})
	return s
}
