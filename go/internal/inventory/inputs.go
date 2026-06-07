// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"regexp"
	"strings"
)

var (
	inputNameRE    = regexp.MustCompile(`^N: Name="(.*)"`)
	inputHandlerRE = regexp.MustCompile(`(?i)^H: Handlers=(\w+)`)
)

// ParseInputDevices parses /proc/bus/input/devices into the INPUTS section,
// mirroring Linux/Inputs.pm: a device is emitted when its Phys is an "input"
// (not a "button") device; TYPE is Keyboard/Pointing derived from the first
// handler.
func ParseInputDevices(r io.Reader) []map[string]any {
	var inputs []map[string]any
	var name, phys, typ string

	flush := func() {
		if phys == "input" && name != "" {
			entry := map[string]any{"DESCRIPTION": name, "CAPTION": name}
			setIf(entry, "TYPE", typ)
			inputs = append(inputs, entry)
		}
		name, phys, typ = "", "", ""
	}

	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		line := scanner.Text()
		switch {
		case strings.TrimSpace(line) == "":
			flush()
		case strings.HasPrefix(line, "P: Phys="):
			lower := strings.ToLower(line)
			if strings.Contains(lower, "button") {
				phys = "nodev"
			} else if strings.Contains(lower, "input") {
				phys = "input"
			}
		default:
			if m := inputNameRE.FindStringSubmatch(line); m != nil {
				name = m[1]
			}
			if m := inputHandlerRE.FindStringSubmatch(line); m != nil {
				switch h := strings.ToLower(m[1]); {
				case strings.Contains(h, "kbd"):
					typ = "Keyboard"
				case strings.Contains(h, "mouse"):
					typ = "Pointing"
				default:
					typ = m[1]
				}
			}
		}
	}
	flush() // final block if the file does not end with a blank line
	return inputs
}
