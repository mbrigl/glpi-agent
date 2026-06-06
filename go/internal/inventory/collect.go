// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "os"

// Sections is the collected local inventory, keyed by canonical UPPERCASE
// section name (HARDWARE, OPERATINGSYSTEM, CPUS, …), ready to merge into the
// content model.
type Sections map[string]any

// hostname returns the local hostname for HARDWARE.NAME (Generic/Hostname.pm).
func hostname() string {
	h, err := os.Hostname()
	if err != nil || h == "" {
		return "localhost"
	}
	return h
}

// mergeHardware merges fields into the HARDWARE section of s, creating it if
// needed.
func (s Sections) mergeHardware(fields map[string]any) {
	hw, ok := s["HARDWARE"].(map[string]any)
	if !ok {
		hw = map[string]any{}
		s["HARDWARE"] = hw
	}
	for k, v := range fields {
		hw[k] = v
	}
}
