// SPDX-License-Identifier: GPL-2.0-only

//go:build linux

package inventory

import (
	"os"
	"strings"
)

// Collect gathers the local Linux inventory: OPERATINGSYSTEM, HARDWARE (name +
// memory) and CPUS. It reads the same sources as the upstream Perl modules
// (/etc/os-release, /proc/sys/kernel/osrelease, /proc/meminfo, /proc/cpuinfo).
func Collect() Sections {
	s := Sections{}

	// OPERATINGSYSTEM: os-release distro fields + kernel name/version.
	os := map[string]any{"KERNEL_NAME": "linux"}
	if f, err := osOpen("/etc/os-release"); err == nil {
		for k, v := range ParseOSRelease(f) {
			os[k] = v
		}
		f.Close()
	}
	if rel := firstLine("/proc/sys/kernel/osrelease"); rel != "" {
		os["KERNEL_VERSION"] = rel
	}
	s["OPERATINGSYSTEM"] = os

	// HARDWARE: hostname + memory/swap.
	s.mergeHardware(map[string]any{"NAME": hostname()})
	if f, err := osOpen("/proc/meminfo"); err == nil {
		mem, swap := ParseMemInfo(f)
		f.Close()
		hw := map[string]any{}
		if mem > 0 {
			hw["MEMORY"] = mem
		}
		if swap > 0 {
			hw["SWAP"] = swap
		}
		s.mergeHardware(hw)
	}

	// CPUS.
	if f, err := osOpen("/proc/cpuinfo"); err == nil {
		if cpus := ParseCPUInfo(f); len(cpus) > 0 {
			s["CPUS"] = cpus
		}
		f.Close()
	}

	return s
}

func osOpen(path string) (*os.File, error) { return os.Open(path) }

func firstLine(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(strings.SplitN(string(data), "\n", 2)[0])
}
