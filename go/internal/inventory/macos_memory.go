// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/hex"
	"regexp"
	"sort"
	"strings"
)

var macDIMMRE = regexp.MustCompile(`DIMM(\d)`)

// buildMacMemories maps SPMemoryDataType to the MEMORIES section, mirroring
// MacOS/Memory.pm: one entry per DIMM slot (NUMSLOTS/CAPTION/TYPE/SERIALNUMBER/
// SPEED/CAPACITY/DESCRIPTION), with the Apple-Silicon integrated-memory fallback
// when there are no DIMM slots.
func buildMacMemories(memInfos map[string]any) []map[string]any {
	memory := spNode(memInfos, "Memory")
	if memory == nil {
		return nil
	}
	parent := memory
	if slots := spNode(memInfos, "Memory", "Memory Slots"); slots != nil {
		parent = slots
	}

	keys := make([]string, 0, len(parent))
	for k := range parent {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	var memories []map[string]any
	for _, key := range keys {
		m := macDIMMRE.FindStringSubmatch(key)
		if m == nil {
			continue
		}
		info, ok := parent[key].(map[string]any)
		if !ok {
			continue
		}

		mem := map[string]any{
			"NUMSLOTS": m[1],
			"CAPTION":  "Status: " + spLeaf(info, "Status"),
		}
		setIf(mem, "TYPE", spLeaf(info, "Type"))
		setIf(mem, "SERIALNUMBER", spLeaf(info, "Serial Number"))
		if speed := canonicalSpeed(spLeaf(info, "Speed")); speed > 0 {
			mem["SPEED"] = speed
		}
		if cap := canonicalSizeMB(spLeaf(info, "Size")); cap > 0 {
			mem["CAPACITY"] = cap
		}
		if desc := macMemoryDescription(spLeaf(info, "Part Number")); desc != "" {
			mem["DESCRIPTION"] = desc
		}
		memories = append(memories, mem)
	}

	// Apple Silicon: integrated memory reported directly under the Memory node.
	if len(memories) == 0 {
		size := spLeaf(parent, "Memory")
		typ := spLeaf(parent, "Type")
		if size != "" && typ != "" {
			mem := map[string]any{
				"NUMSLOTS":    "0",
				"DESCRIPTION": "Integrated memory",
				"TYPE":        typ,
			}
			if cap := canonicalSizeMB(size); cap > 0 {
				mem["CAPACITY"] = cap
			}
			memories = append(memories, mem)
		}
	}

	return memories
}

// macMemoryDescription decodes a DIMM Part Number, converting a "0x..." hex
// string to ASCII (MacOS/Memory.pm).
func macMemoryDescription(part string) string {
	if part == "" {
		return ""
	}
	if strings.HasPrefix(part, "0x") {
		if b, err := hex.DecodeString(part[2:]); err == nil {
			return strings.TrimRight(string(b), " \x00")
		}
	}
	return part
}

// macTotalMemoryMB returns the hardware MEMORY size from SPMemoryDataType,
// mirroring MacOS/Memory.pm _getMemory.
func macTotalMemoryMB(memInfos map[string]any) int {
	return canonicalSizeMB(spString(memInfos, "Hardware", "Hardware Overview", "Memory"))
}
