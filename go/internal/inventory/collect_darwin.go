// SPDX-License-Identifier: GPL-2.0-only

//go:build darwin

package inventory

import (
	"os/exec"
	"regexp"
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

	// cpus (SPHardwareDataType "Hardware Overview" + sysctl machdep.cpu).
	overview := spNode(hardware, "Hardware", "Hardware Overview")
	sysctl := parseSysctl(commandOutput("sysctl", "-a", "machdep.cpu"))
	if cpus := buildMacCPUs(overview, sysctl); len(cpus) > 0 {
		s["CPUS"] = cpus
	}

	// memories (SPMemoryDataType) + hardware MEMORY (from SPHardwareDataType).
	memory := systemProfiler("SPMemoryDataType")
	if mems := buildMacMemories(memory); len(mems) > 0 {
		s["MEMORIES"] = mems
	}
	if total := macTotalMemoryMB(hardware); total > 0 {
		s.mergeHardware(map[string]any{"MEMORY": total})
	}

	// bios (SPHardwareDataType + ioreg IOPlatformExpertDevice).
	s["BIOS"] = buildMacBios(overview, ioregDevice())

	// videos (SPDisplaysDataType).
	if v := buildMacVideos(systemProfiler("SPDisplaysDataType")); len(v) > 0 {
		s["VIDEOS"] = v
	}

	// powersupplies (SPPowerDataType AC Charger Information).
	if psu := buildMacCharger(systemProfiler("SPPowerDataType")); psu != nil {
		s["POWERSUPPLIES"] = []map[string]any{psu}
	}

	// networks (ifconfig joined with networksetup hardware ports).
	netsetup := parseMacNetworkSetup(commandOutput("networksetup", "-listallhardwareports"))
	if n := buildMacNetworks(commandOutput("/sbin/ifconfig", "-a"), netsetup); len(n) > 0 {
		s["NETWORKS"] = n
	}

	return s
}

// ioregDevice returns the IOPlatformExpertDevice attributes as a flat map.
func ioregDevice() map[string]any {
	out, err := exec.Command("ioreg", "-rd1", "-c", "IOPlatformExpertDevice").Output()
	if err != nil {
		return nil
	}
	dev := map[string]any{}
	re := regexp.MustCompile(`"([^"]+)"\s*=\s*(?:<?)"?([^"<>]*)"?`)
	for _, line := range strings.Split(string(out), "\n") {
		if m := re.FindStringSubmatch(line); m != nil {
			dev[m[1]] = strings.TrimSpace(m[2])
		}
	}
	return dev
}

// commandOutput runs a command and returns its stdout (empty on error).
func commandOutput(name string, args ...string) string {
	out, err := exec.Command(name, args...).Output()
	if err != nil {
		return ""
	}
	return string(out)
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
