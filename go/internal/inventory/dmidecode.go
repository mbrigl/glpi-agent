// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"regexp"
	"strconv"
	"strings"
)

// DmiRecord is one dmidecode structure: its DMI type and its "Key: Value" pairs.
type DmiRecord struct {
	Type   int
	Fields map[string]string
}

var dmiHandleRE = regexp.MustCompile(`DMI type (\d+),`)

// ParseDmidecode parses `dmidecode` text output into records grouped by DMI
// type, mirroring the structure GLPI::Agent::Tools::Generic::getDmidecodeInfos
// builds. Each record's first non-blank line is its name; subsequent indented
// "Key: Value" lines are its fields.
func ParseDmidecode(r io.Reader) map[int][]DmiRecord {
	byType := map[int][]DmiRecord{}
	var cur *DmiRecord

	flush := func() {
		if cur != nil && len(cur.Fields) > 0 {
			byType[cur.Type] = append(byType[cur.Type], *cur)
		}
		cur = nil
	}

	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for scanner.Scan() {
		line := scanner.Text()
		if strings.HasPrefix(line, "Handle ") {
			flush()
			if m := dmiHandleRE.FindStringSubmatch(line); m != nil {
				t, _ := strconv.Atoi(m[1])
				cur = &DmiRecord{Type: t, Fields: map[string]string{}}
			}
			continue
		}
		if cur == nil {
			continue
		}
		if strings.TrimSpace(line) == "" {
			flush()
			continue
		}
		// Indented "Key: Value" fields; non-indented lines are the structure name.
		if line[0] == '\t' || line[0] == ' ' {
			if k, v, ok := strings.Cut(strings.TrimSpace(line), ":"); ok {
				cur.Fields[strings.TrimSpace(k)] = strings.TrimSpace(v)
			}
		}
	}
	flush()
	return byType
}

// BuildMemories assembles the MEMORIES section from dmidecode type 17 (Memory
// Device) records, mirroring Generic/Dmidecode/Memory.pm. MEMORYCORRECTION comes
// from the type 16 Physical Memory Array.
func BuildMemories(byType map[int][]DmiRecord) []map[string]any {
	var correction string
	if arrays := byType[16]; len(arrays) > 0 {
		correction = arrays[0].Fields["Error Correction Type"]
	}

	var memories []map[string]any
	for i, rec := range byType[17] {
		info := rec.Fields
		memory := map[string]any{"NUMSLOTS": i + 1}
		setIf(memory, "CAPTION", info["Locator"])
		setIf(memory, "DESCRIPTION", info["Form Factor"])
		setIf(memory, "TYPE", info["Type"])
		setIf(memory, "SERIALNUMBER", info["Serial Number"])
		setIf(memory, "MANUFACTURER", info["Manufacturer"])
		setIf(memory, "MEMORYCORRECTION", correction)
		if speed := canonicalSpeed(info["Speed"]); speed > 0 {
			memory["SPEED"] = speed
		}
		if cap := canonicalSizeMB(info["Size"]); cap > 0 {
			memory["CAPACITY"] = cap
		}
		memories = append(memories, memory)
	}
	return memories
}

var sizeRE = regexp.MustCompile(`(?i)^(\d+)\s*([kmgt]?i?b)$`)

// canonicalSizeMB converts a dmidecode size like "16 GB" / "8192 MB" to MiB,
// mirroring getCanonicalSize(.., 1024).
func canonicalSizeMB(s string) int {
	m := sizeRE.FindStringSubmatch(strings.TrimSpace(s))
	if m == nil {
		return 0
	}
	n, _ := strconv.Atoi(m[1])
	switch strings.ToLower(strings.TrimSuffix(m[2], "b"))[:1] {
	case "k":
		return n / 1024
	case "m":
		return n
	case "g":
		return n * 1024
	case "t":
		return n * 1024 * 1024
	default:
		return n
	}
}

var speedRE = regexp.MustCompile(`(\d+)`)

// canonicalSpeed extracts the numeric speed from "3200 MT/s" / "2133 MHz",
// mirroring getCanonicalSpeed.
func canonicalSpeed(s string) int {
	if m := speedRE.FindStringSubmatch(s); m != nil {
		n, _ := strconv.Atoi(m[1])
		return n
	}
	return 0
}
