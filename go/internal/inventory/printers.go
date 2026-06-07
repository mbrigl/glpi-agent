// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"regexp"
	"strings"
)

var printerHeaderRE = regexp.MustCompile(`^<(?:Default)?Printer\s+(\S+)>`)

// ParsePrintersConf parses /etc/cups/printers.conf into the PRINTERS section,
// mirroring the CUPS data Generic/Printers.pm reads via Net::CUPS: NAME, PORT
// (DeviceURI), DESCRIPTION (Info) and DRIVER (MakeModel).
func ParsePrintersConf(r io.Reader) []map[string]any {
	var printers []map[string]any
	var cur map[string]any

	flush := func() {
		if cur != nil {
			printers = append(printers, cur)
		}
		cur = nil
	}

	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if m := printerHeaderRE.FindStringSubmatch(line); m != nil {
			flush()
			cur = map[string]any{"NAME": m[1]}
			continue
		}
		if cur == nil {
			continue
		}
		if line == "</Printer>" {
			flush()
			continue
		}
		key, val, ok := strings.Cut(line, " ")
		if !ok {
			continue
		}
		switch key {
		case "DeviceURI":
			cur["PORT"] = val
		case "Info":
			cur["DESCRIPTION"] = val
		case "MakeModel":
			cur["DRIVER"] = val
		}
	}
	flush()
	return printers
}
