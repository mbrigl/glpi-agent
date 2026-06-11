// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import (
	"os"
	"regexp"
	"strings"

	"golang.org/x/sys/windows/registry"
)

const winOfficePath = `SOFTWARE\Microsoft\Office`

// winAdobeCachePaths are the Adobe PCD license cache locations (Win32/License.pm):
// the 64-bit host keeps it under "Program Files (x86)", the 32-bit one under
// "Program Files".
var winAdobeCachePaths = []string{
	`C:\Program Files (x86)\Common Files\Adobe\Adobe PCD\cache\cache.db`,
	`C:\Program Files\Common Files\Adobe\Adobe PCD\cache\cache.db`,
}

// winRegWordHyphenRE extracts the "clean" identifier (word chars + hyphens) from
// a braced registry UUID / product code, mirroring the Perl /([-\w]+)/ capture.
var winRegWordHyphenRE = regexp.MustCompile(`[-\w]+`)

// collectWinLicenses builds the LICENSEINFOS section: it scans the Office
// registration registry (both the 64-bit and 32-bit views) for product keys,
// then folds in the SoftwareLicensingProduct WMI source (Win32/License.pm). The
// Adobe cache.db source is follow-on.
func collectWinLicenses() []map[string]any {
	seen := map[string]map[string]any{}
	for _, access := range []uint32{registry.READ | registry.WOW64_64KEY, registry.READ | registry.WOW64_32KEY} {
		scanOfficeRegistry(seen, access)
	}

	wmi, _ := powershellCIM("SoftwareLicensingProduct", winLicenseProperties)
	licenses := collectWinAdobeLicenses()
	return append(licenses, mergeWinLicenses(seen, wmi)...)
}

// collectWinAdobeLicenses reads the first Adobe PCD cache.db that exists and
// parses its licenses (Win32/License.pm getAdobeLicensesWithoutSqlite).
func collectWinAdobeLicenses() []map[string]any {
	for _, path := range winAdobeCachePaths {
		if data, err := os.ReadFile(path); err == nil && len(data) > 0 {
			return parseAdobeLicenses(data)
		}
	}
	return nil
}

// scanOfficeRegistry walks Office/<version>/Registration/<UUID> and records the
// product licenses / product-code stubs into seen (Win32/License.pm
// _scanOfficeLicences).
func scanOfficeRegistry(seen map[string]map[string]any, access uint32) {
	office, err := registry.OpenKey(registry.LOCAL_MACHINE, winOfficePath, access|registry.ENUMERATE_SUB_KEYS)
	if err != nil {
		return
	}
	defer office.Close()

	versions, _ := office.ReadSubKeyNames(-1)
	for _, version := range versions {
		regPath := winOfficePath + `\` + version + `\Registration`
		reg, err := registry.OpenKey(registry.LOCAL_MACHINE, regPath, access|registry.ENUMERATE_SUB_KEYS)
		if err != nil {
			continue
		}
		uuids, _ := reg.ReadSubKeyNames(-1)
		reg.Close()

		for _, uuid := range uuids {
			values, dpid := readOfficeEntry(regPath+`\`+uuid, access)
			cleanUUID := strings.ToLower(winRegWordHyphenRE.FindString(uuid))
			if cleanUUID == "" {
				continue
			}

			if len(dpid) > 0 {
				seen[cleanUUID] = buildOfficeLicense(values, dpid)
			}
			productName := values["ProductName"]
			if values["ProductCode"] != "" && productName != "" {
				stub := map[string]any{
					"PRODUCTCODE": strings.ToLower(winRegWordHyphenRE.FindString(values["ProductCode"])),
					"FULLNAME":    productName,
				}
				if regexp.MustCompile(`(?i)trial`).MatchString(values["ProductNameBrand"]) {
					stub["TRIAL"] = 1
				}
				seen[cleanUUID] = stub
			}
		}
	}
}

// readOfficeEntry reads the string values (and the binary DigitalProductID) of a
// single Office registration subkey.
func readOfficeEntry(path string, access uint32) (map[string]string, []byte) {
	key, err := registry.OpenKey(registry.LOCAL_MACHINE, path, access|registry.QUERY_VALUE)
	if err != nil {
		return nil, nil
	}
	defer key.Close()

	values := map[string]string{}
	names, _ := key.ReadValueNames(-1)
	for _, name := range names {
		if s, _, err := key.GetStringValue(name); err == nil {
			values[name] = s
		} else {
			// Keep the value name (with empty value) so component detection
			// (<App>NameVersion) still works for non-string values.
			values[name] = ""
		}
	}

	var dpid []byte
	if b, _, err := key.GetBinaryValue("DigitalProductID"); err == nil {
		dpid = b
	}
	return values, dpid
}
