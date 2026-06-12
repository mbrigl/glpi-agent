// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strconv"
	"strings"
)

// parseSysctl parses `sysctl -a machdep.cpu` output ("key: value" lines) into a
// map, mirroring the parser in MacOS/CPU.pm.
func parseSysctl(text string) map[string]string {
	out := map[string]string{}
	for _, line := range strings.Split(text, "\n") {
		i := strings.IndexByte(line, ':')
		if i < 0 {
			continue
		}
		rest := line[i+1:]
		if rest == "" || (rest[0] != ' ' && rest[0] != '\t') {
			continue
		}
		key := line[:i]
		value := rest[1:]
		if value != "" {
			out[key] = value
		}
	}
	return out
}

var (
	macSpeedGHzRE = regexp.MustCompile(`(?i)GHz$`)
	macSpeedMHzRE = regexp.MustCompile(`(?i)MHz$`)
	macWSRE       = regexp.MustCompile(`\s+`)
)

// trimWhitespace trims and collapses internal whitespace, mirroring
// Tools.pm trimWhitespace.
func trimWhitespace(s string) string {
	return macWSRE.ReplaceAllString(strings.TrimSpace(s), " ")
}

// macCPUSpeed normalises a system_profiler CPU speed ("2,26 GHz") to MHz,
// mirroring MacOS/CPU.pm (",→.", GHz→*1000, MHz strip, whitespace removed).
func macCPUSpeed(s string) int {
	s = strings.ReplaceAll(s, ",", ".")
	switch {
	case macSpeedGHzRE.MatchString(s):
		s = macSpeedGHzRE.ReplaceAllString(s, "")
		if f, err := strconv.ParseFloat(strings.TrimSpace(s), 64); err == nil {
			return int(f * 1000)
		}
	case macSpeedMHzRE.MatchString(s):
		s = macSpeedMHzRE.ReplaceAllString(s, "")
		if f, err := strconv.ParseFloat(strings.TrimSpace(s), 64); err == nil {
			return int(f)
		}
	}
	return 0
}

// buildMacCPUs maps the SPHardwareDataType "Hardware Overview" plus the parsed
// `sysctl machdep.cpu` map to the CPUS section, mirroring MacOS/CPU.pm: the CPU
// entry is repeated once per processor. CORE comes from "Total Number Of Cores"
// divided by the processor count (or sysctl core_count); FAMILYNUMBER/MODEL/
// STEPPING/THREAD come from sysctl; SPEED is normalised to MHz.
func buildMacCPUs(overview map[string]any, sysctl map[string]string) []map[string]any {
	cpuType := winFirstNonEmpty(sysctl["machdep.cpu.brand_string"],
		spLeaf(overview, "Processor Name"), spLeaf(overview, "CPU Type"))

	procs := atoiOr(winFirstNonEmpty(spLeaf(overview, "Number Of Processors"),
		spLeaf(overview, "Number Of CPUs")), 1)
	if procs < 1 {
		procs = 1
	}

	speed := macCPUSpeed(winFirstNonEmpty(spLeaf(overview, "Processor Speed"), spLeaf(overview, "CPU Speed")))

	var cores string
	if total := spLeaf(overview, "Total Number Of Cores"); total != "" {
		cores = strconv.Itoa(atoiOr(total, 0) / procs)
	} else {
		cores = sysctl["machdep.cpu.core_count"]
	}

	cpu := map[string]any{
		"CORE": cores,
		"NAME": trimWhitespace(cpuType),
	}
	setIf(cpu, "MANUFACTURER", macCPUManufacturer(cpuType))
	setIf(cpu, "THREAD", sysctl["machdep.cpu.thread_count"])
	setIf(cpu, "FAMILYNUMBER", sysctl["machdep.cpu.family"])
	setIf(cpu, "MODEL", sysctl["machdep.cpu.model"])
	setIf(cpu, "STEPPING", sysctl["machdep.cpu.stepping"])
	if speed > 0 {
		cpu["SPEED"] = speed
	}

	cpus := make([]map[string]any, 0, procs)
	for i := 0; i < procs; i++ {
		cpus = append(cpus, cpu)
	}
	return cpus
}

// macCPUManufacturer maps the CPU type string to a manufacturer (MacOS/CPU.pm).
func macCPUManufacturer(t string) string {
	switch {
	case regexp.MustCompile(`(?i)Intel`).MatchString(t):
		return "Intel"
	case regexp.MustCompile(`(?i)AMD`).MatchString(t):
		return "AMD"
	case regexp.MustCompile(`(?i)Apple`).MatchString(t):
		return "Apple"
	}
	return ""
}

// spLeaf returns a string leaf directly under a node, "" when absent.
func spLeaf(node map[string]any, key string) string {
	if node == nil {
		return ""
	}
	s, _ := node[key].(string)
	return strings.TrimSpace(s)
}

// atoiOr parses the leading integer of s, returning def when there is none.
func atoiOr(s string, def int) int {
	m := regexp.MustCompile(`-?\d+`).FindString(strings.TrimSpace(s))
	if m == "" {
		return def
	}
	n, err := strconv.Atoi(m)
	if err != nil {
		return def
	}
	return n
}
