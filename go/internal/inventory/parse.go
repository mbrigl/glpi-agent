// SPDX-License-Identifier: GPL-2.0-only

// Package inventory collects the local machine inventory.
//
// Derived from the upstream Perl modules under
// lib/GLPI/Agent/Task/Inventory/{Generic,Linux}: the OPERATINGSYSTEM section
// (Linux/OS.pm + Generic/OS.pm + Linux/Distro/OSRelease.pm), HARDWARE memory
// (Linux/Memory.pm), and the CPUS section (Linux/i386/CPU.pm /proc/cpuinfo
// parsing). The pure parsers here are OS-independent and fixture-testable; the
// platform wiring that opens the real /proc and /etc files is build-tagged
// (collect_linux.go / collect_other.go).
package inventory

import (
	"bufio"
	"io"
	"strconv"
	"strings"
)

// ParseOSRelease maps /etc/os-release to the OPERATINGSYSTEM fields, mirroring
// Linux/Distro/OSRelease.pm::_getOSRelease (NAME, VERSION, PRETTY_NAME).
func ParseOSRelease(r io.Reader) map[string]any {
	os := map[string]any{}
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		key, val, ok := strings.Cut(scanner.Text(), "=")
		if !ok {
			continue
		}
		val = strings.Trim(strings.TrimSpace(val), `"`)
		switch strings.TrimSpace(key) {
		case "NAME":
			os["NAME"] = val
		case "VERSION":
			os["VERSION"] = val
		case "PRETTY_NAME":
			os["FULL_NAME"] = val
		}
	}
	return os
}

// ParseMemInfo returns total memory and swap in MiB from /proc/meminfo,
// mirroring Linux/Memory.pm (MemTotal/1024, SwapTotal/1024).
func ParseMemInfo(r io.Reader) (memoryMB, swapMB int) {
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}
		kb, err := strconv.Atoi(fields[1])
		if err != nil {
			continue
		}
		switch fields[0] {
		case "MemTotal:":
			memoryMB = kb / 1024
		case "SwapTotal:":
			swapMB = kb / 1024
		}
	}
	return memoryMB, swapMB
}

// ParseCPUInfo parses /proc/cpuinfo into the CPUS entries, mirroring
// Linux/i386/CPU.pm: logical processors are grouped by "physical id", CORE is
// "cpu cores" and THREAD is siblings/cores.
func ParseCPUInfo(r io.Reader) []map[string]any {
	logical := splitLogicalCPUs(r)

	var cpus []map[string]any
	seen := map[string]bool{}
	count := 0
	for _, lc := range logical {
		physID, hasPhys := lc["physical id"]
		core, thread := 1, 1
		if hasPhys {
			if seen[physID] {
				continue
			}
			seen[physID] = true
			if c, err := strconv.Atoi(lc["cpu cores"]); err == nil && c > 0 {
				core = c
			}
			if s, err := strconv.Atoi(lc["siblings"]); err == nil && core > 0 && s >= core {
				thread = s / core
			}
		} else {
			physID = strconv.Itoa(count)
		}
		count++

		cpu := map[string]any{
			"ARCH":   "i386",
			"CORE":   core,
			"THREAD": thread,
		}
		setIf(cpu, "MANUFACTURER", canonicalManufacturer(lc["vendor_id"]))
		setIf(cpu, "NAME", strings.TrimSpace(lc["model name"]))
		setIf(cpu, "STEPPING", lc["stepping"])
		setIf(cpu, "FAMILYNUMBER", lc["cpu family"])
		setIf(cpu, "MODEL", lc["model"])
		if mhz := lc["cpu MHz"]; mhz != "" {
			if f, err := strconv.ParseFloat(mhz, 64); err == nil {
				cpu["SPEED"] = int(f + 0.5)
			}
		}
		cpus = append(cpus, cpu)
	}
	return cpus
}

// splitLogicalCPUs groups /proc/cpuinfo into per-processor key/value maps.
func splitLogicalCPUs(r io.Reader) []map[string]string {
	var all []map[string]string
	current := map[string]string{}
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		line := scanner.Text()
		if strings.TrimSpace(line) == "" {
			if len(current) > 0 {
				all = append(all, current)
				current = map[string]string{}
			}
			continue
		}
		key, val, ok := strings.Cut(line, ":")
		if !ok {
			continue
		}
		current[strings.TrimSpace(key)] = strings.TrimSpace(val)
	}
	if len(current) > 0 {
		all = append(all, current)
	}
	return all
}

// canonicalManufacturer maps a CPU vendor_id to a friendly name, mirroring the
// common cases of GLPI::Agent::Tools::getCanonicalManufacturer.
func canonicalManufacturer(vendorID string) string {
	switch vendorID {
	case "":
		return ""
	case "GenuineIntel":
		return "Intel"
	case "AuthenticAMD", "AMDisbetter!":
		return "AMD"
	case "CentaurHauls":
		return "VIA"
	default:
		return vendorID
	}
}

func setIf(m map[string]any, key, val string) {
	if val != "" {
		m[key] = val
	}
}
