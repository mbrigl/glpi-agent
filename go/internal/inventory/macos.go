// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strconv"
	"strings"
)

// macOS inventory is collected by running `system_profiler <SPxxxDataType>`,
// `sysctl`, `ioreg` and a few other commands and mapping their output to the
// GLPI sections, mirroring the upstream Task/Inventory/MacOS/* modules. The
// command runners live in the //go:build darwin collector; the parsing here is
// pure and unit-tested on any platform against vendored fixtures.

var spLineRE = regexp.MustCompile(`^(\s*)(\S[^:]*):(?: (.*\S))?`)

// spFrame is one node on the indentation stack used by parseSystemProfiler.
type spFrame struct {
	node  map[string]any
	level int
	key   string
}

// parseSystemProfiler parses the indented "key: value" text output of
// `system_profiler` into a nested map (string leaves, map[string]any nodes),
// mirroring Tools/MacOS.pm getSystemProfilerInfos. Duplicate sibling section
// names are disambiguated with a "_<n>" suffix.
func parseSystemProfiler(text string) map[string]any {
	info := map[string]any{}
	parents := []spFrame{{node: info, level: -1}}

	for _, line := range strings.Split(text, "\n") {
		m := spLineRE.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		level := len(m[1])
		key := m[2]
		value := m[3]
		hasValue := value != ""

		parentLevel := parents[len(parents)-1].level

		if hasValue {
			if level <= parentLevel {
				top := parents[len(parents)-1]
				if len(top.node) == 0 {
					// Discard the just-created empty node from its parent.
					parents[len(parents)-2].node[top.key] = nil
				}
				for len(parents) > 1 && level <= parents[len(parents)-1].level {
					parents = parents[:len(parents)-1]
				}
			}
			if key == "Last Modified" {
				value = macFormatDate(value)
			}
			parents[len(parents)-1].node[key] = value
			continue
		}

		// Section header (no value): position the stack at the right depth.
		switch {
		case level > parentLevel:
			// Going deeper: keep the current parent.
		case level < parentLevel:
			for len(parents) > 1 && level <= parents[len(parents)-1].level {
				parents = parents[:len(parents)-1]
			}
		default:
			if len(parents) > 1 {
				parents = parents[:len(parents)-1]
			}
		}

		parentNode := parents[len(parents)-1].node
		k := key
		for i := 0; ; i++ {
			if v, ok := parentNode[k]; !ok || v == nil {
				break
			}
			k = key + "_" + strconv.Itoa(i)
		}
		child := map[string]any{}
		parentNode[k] = child
		parents = append(parents, spFrame{node: child, level: level, key: k})
	}

	return info
}

// spNode walks a path of section names and returns the nested node, or nil.
func spNode(info map[string]any, path ...string) map[string]any {
	cur := info
	for _, p := range path {
		next, ok := cur[p].(map[string]any)
		if !ok {
			return nil
		}
		cur = next
	}
	return cur
}

// spString returns a string leaf at the given path (last element is the key).
func spString(info map[string]any, path ...string) string {
	if len(path) == 0 {
		return ""
	}
	node := spNode(info, path[:len(path)-1]...)
	if node == nil {
		return ""
	}
	s, _ := node[path[len(path)-1]].(string)
	return strings.TrimSpace(s)
}

var macSystemVersionRE = regexp.MustCompile(`^(.*?)\s+(\d+.*)$`)

// macSystemVersion splits a "System Version" string ("macOS 11.2.3 (20D91)")
// into its name ("macOS") and version ("11.2.3 (20D91)") parts.
func macSystemVersion(s string) (name, version string) {
	if m := macSystemVersionRE.FindStringSubmatch(strings.TrimSpace(s)); m != nil {
		return m[1], m[2]
	}
	return "", ""
}

// buildMacOS maps the SPSoftwareDataType "System Version" to the os fields that
// come from system_profiler (FULL_NAME/VERSION), mirroring MacOS/OS.pm; NAME is
// fixed to "MacOSX" and the live KERNEL_VERSION/ARCH/BOOT_TIME/INSTALL_DATE are
// added by the darwin collector.
func buildMacOS(softwareInfos map[string]any) map[string]any {
	os := map[string]any{"NAME": "MacOSX"}
	version := spString(softwareInfos, "Software", "System Software Overview", "System Version")
	if full, ver := macSystemVersion(version); full != "" {
		os["FULL_NAME"] = full
		os["VERSION"] = ver
	}
	return os
}

// buildMacHardware maps SPSoftwareDataType + SPHardwareDataType to the hardware
// section, mirroring MacOS/Hardware.pm: NAME from the System Version product
// name (default "Mac OS X") and UUID from the Hardware UUID. The ioreg UUID
// fallback is handled by the darwin collector.
func buildMacHardware(softwareInfos, hardwareInfos map[string]any) map[string]any {
	hardware := map[string]any{"NAME": "Mac OS X"}
	if full, _ := macSystemVersion(spString(softwareInfos, "Software", "System Software Overview", "System Version")); full != "" {
		hardware["NAME"] = full
	}
	overview := spNode(hardwareInfos, "Hardware", "Hardware Overview")
	if overview != nil {
		if uuid, _ := overview["Hardware UUID"].(string); uuid != "" {
			hardware["UUID"] = strings.TrimSpace(uuid)
		}
	}
	return hardware
}
