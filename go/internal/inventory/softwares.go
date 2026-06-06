// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"strconv"
	"strings"
)

// ParseDpkgStatus parses a dpkg status database (/var/lib/dpkg/status) into the
// SOFTWARES entries, mirroring Generic/Softwares/Deb.pm: NAME/ARCH/VERSION/
// FILESIZE/SYSTEM_CATEGORY/FROM, keeping only installed packages.
func ParseDpkgStatus(r io.Reader) []map[string]any {
	var softwares []map[string]any
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)

	fields := map[string]string{}
	flush := func() {
		defer func() { fields = map[string]string{} }()
		if !strings.HasSuffix(fields["Status"], " installed") {
			return // not an installed package
		}
		name := fields["Package"]
		if name == "" {
			return
		}
		entry := map[string]any{"NAME": name, "FROM": "deb"}
		setIf(entry, "ARCH", fields["Architecture"])
		setIf(entry, "VERSION", fields["Version"])
		setIf(entry, "SYSTEM_CATEGORY", fields["Section"])
		if sz, err := strconv.Atoi(fields["Installed-Size"]); err == nil {
			entry["FILESIZE"] = sz * 1024 // dpkg reports KiB; GLPI wants bytes
		}
		softwares = append(softwares, entry)
	}

	var lastKey string
	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			flush()
			lastKey = ""
			continue
		}
		if line[0] == ' ' || line[0] == '\t' {
			continue // folded continuation line (e.g. Description body)
		}
		key, val, ok := strings.Cut(line, ":")
		if !ok {
			continue
		}
		lastKey = strings.TrimSpace(key)
		fields[lastKey] = strings.TrimSpace(val)
	}
	flush() // final paragraph (file may not end with a blank line)
	return softwares
}
