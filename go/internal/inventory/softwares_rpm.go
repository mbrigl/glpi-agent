// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strconv"
	"strings"
	"time"
)

// rpmQueryFormat is the rpm --queryformat used by collect_linux, matching
// Generic/Softwares/RPM.pm.
const rpmQueryFormat = `%{NAME}\t%{ARCH}\t%{VERSION}-%{RELEASE}\t%{INSTALLTIME}\t%{SIZE}\t%{VENDOR}\t%{SUMMARY}\t%{GROUP}\n`

// ParseRPMQA parses `rpm -qa --queryformat '<rpmQueryFormat>'` output into the
// SOFTWARES section, mirroring Generic/Softwares/RPM.pm: NAME/ARCH/VERSION/
// FILESIZE/COMMENTS/SYSTEM_CATEGORY/FROM, INSTALLDATE from the install epoch and
// PUBLISHER from VENDOR (unless "(none)").
func ParseRPMQA(out string) []map[string]any {
	var softwares []map[string]any
	for _, line := range strings.Split(out, "\n") {
		if line == "" {
			continue
		}
		f := strings.Split(line, "\t")
		if len(f) < 8 || f[0] == "" {
			continue
		}
		pkg := map[string]any{"NAME": f[0], "FROM": "rpm"}
		setIf(pkg, "ARCH", f[1])
		setIf(pkg, "VERSION", f[2])
		setIf(pkg, "COMMENTS", f[6])
		setIf(pkg, "SYSTEM_CATEGORY", f[7])
		if size, err := strconv.Atoi(strings.TrimSpace(f[4])); err == nil {
			pkg["FILESIZE"] = size
		}
		if epoch, err := strconv.ParseInt(strings.TrimSpace(f[3]), 10, 64); err == nil && epoch > 0 {
			pkg["INSTALLDATE"] = time.Unix(epoch, 0).Format("02/01/2006")
		}
		if vendor := strings.TrimSpace(f[5]); vendor != "" && vendor != "(none)" {
			pkg["PUBLISHER"] = vendor
		}
		softwares = append(softwares, pkg)
	}
	return softwares
}
