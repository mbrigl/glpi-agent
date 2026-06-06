// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestParseDpkgStatus(t *testing.T) {
	const status = `Package: adduser
Status: install ok installed
Priority: important
Section: admin
Installed-Size: 428
Architecture: all
Version: 3.152
Description: add and remove users
 A multi-line description body
 that must be ignored.

Package: half-installed-pkg
Status: install ok half-configured
Architecture: amd64
Version: 1.0

Package: apt
Status: install ok installed
Section: admin
Installed-Size: 4480
Architecture: amd64
Version: 3.0.3
`
	sw := ParseDpkgStatus(strings.NewReader(status))
	if len(sw) != 2 {
		t.Fatalf("got %d packages, want 2 (only installed)", len(sw))
	}

	adduser := sw[0]
	if adduser["NAME"] != "adduser" || adduser["ARCH"] != "all" || adduser["VERSION"] != "3.152" {
		t.Errorf("adduser fields wrong: %v", adduser)
	}
	if adduser["SYSTEM_CATEGORY"] != "admin" || adduser["FROM"] != "deb" {
		t.Errorf("adduser category/from wrong: %v", adduser)
	}
	if adduser["FILESIZE"] != 428*1024 {
		t.Errorf("FILESIZE = %v, want %d bytes", adduser["FILESIZE"], 428*1024)
	}

	if sw[1]["NAME"] != "apt" {
		t.Errorf("second package = %v, want apt", sw[1]["NAME"])
	}
}
