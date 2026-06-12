// SPDX-License-Identifier: GPL-2.0-only

//go:build darwin

package inventory

import (
	"os/exec"
	"strings"
)

// Collect gathers the local macOS inventory via system_profiler, sysctl, ioreg
// and uname, mirroring the upstream Task/Inventory/MacOS/* modules. The parsing
// is pure (macos*.go) and unit-tested on Linux against vendored fixtures.
func Collect() Sections {
	s := Sections{}
	s.mergeHardware(map[string]any{"NAME": hostname()})

	software := systemProfiler("SPSoftwareDataType")
	hardware := systemProfiler("SPHardwareDataType")

	// operatingsystem (system_profiler + uname + boottime).
	os := buildMacOS(software)
	setIf(os, "KERNEL_VERSION", strings.TrimSpace(uname("-r")))
	setIf(os, "ARCH", strings.TrimSpace(uname("-m")))
	s["OPERATINGSYSTEM"] = os

	// hardware (system_profiler; UUID from ioreg as a fallback).
	hw := buildMacHardware(software, hardware)
	if _, ok := hw["UUID"]; !ok {
		if uuid := ioregUUID(); uuid != "" {
			hw["UUID"] = uuid
		}
	}
	s.mergeHardware(hw)

	return s
}

// systemProfiler runs `system_profiler <dataType>` and parses the text output.
func systemProfiler(dataType string) map[string]any {
	out, err := exec.Command("/usr/sbin/system_profiler", dataType).Output()
	if err != nil {
		return map[string]any{}
	}
	return parseSystemProfiler(string(out))
}

// uname runs `uname <flag>` and returns its output.
func uname(flag string) string {
	out, err := exec.Command("uname", flag).Output()
	if err != nil {
		return ""
	}
	return string(out)
}

// ioregUUID reads IOPlatformUUID from the IOPlatformExpertDevice ioreg node.
func ioregUUID() string {
	out, err := exec.Command("ioreg", "-rd1", "-c", "IOPlatformExpertDevice").Output()
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(out), "\n") {
		if strings.Contains(line, "IOPlatformUUID") {
			if i := strings.Index(line, "= "); i >= 0 {
				return strings.Trim(strings.TrimSpace(line[i+2:]), `"`)
			}
		}
	}
	return ""
}
