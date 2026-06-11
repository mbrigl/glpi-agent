// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
)

// winLicenseProperties are the SoftwareLicensingProduct properties for the
// licenseinfo inventory (Win32/License.pm _scanWmiSoftwareLicensingProducts).
var winLicenseProperties = []string{
	"Name", "Description", "LicenseStatus", "PartialProductKey", "ID",
	"ProductKeyChannel", "ProductKeyID", "ProductKeyID2", "ApplicationID",
}

var winLicenseOEMRE = regexp.MustCompile(`(?i)OEM`)
var winLicenseOSRE = regexp.MustCompile(`(?i)Operating System`)

// buildWinLicenses maps SoftwareLicensingProduct (root/CIMV2) objects to
// LICENSEINFOS, mirroring Win32/License.pm _scanWmiSoftwareLicensingProducts +
// _getWmiLicense: only licensed products with a partial product key are kept,
// the OS license is skipped (it comes from the OS module), entries are
// deduplicated by lc(ID) and sorted by NAME/FULLNAME/KEY. The Office-registry
// (DigitalProductID decode) and Adobe cache.db sources are follow-on.
func buildWinLicenses(objects []map[string]any) []map[string]any {
	seen := map[string]map[string]any{}
	var order []string

	for _, o := range objects {
		key := cimString(o, "PartialProductKey")
		if key == "" || cimInt(o, "LicenseStatus") == 0 {
			continue
		}
		if winLicenseOSRE.MatchString(cimString(o, "Description")) {
			continue
		}

		id := strings.ToLower(cimString(o, "ID"))
		if id == "" {
			continue
		}

		if len(key) == 5 {
			key = fmt.Sprintf("XXXXX-XXXXX-XXXXX-XXXXX-%s", key)
		}
		license := map[string]any{
			"KEY": key,
			"OEM": boolToInt(winLicenseOEMRE.MatchString(cimString(o, "ProductKeyChannel"))),
		}
		setIf(license, "PRODUCTID", winFirstNonEmpty(
			cimString(o, "ProductKeyID2"), cimString(o, "ApplicationID"), cimString(o, "ProductKeyID")))
		setIf(license, "FULLNAME", cimString(o, "Description"))
		setIf(license, "NAME", cimString(o, "Name"))

		if _, ok := seen[id]; !ok {
			order = append(order, id)
		}
		seen[id] = license
	}

	licenses := make([]map[string]any, 0, len(order))
	for _, id := range order {
		licenses = append(licenses, seen[id])
	}
	sort.SliceStable(licenses, func(i, j int) bool {
		if a, b := licenseStr(licenses[i], "NAME"), licenseStr(licenses[j], "NAME"); a != b {
			return a < b
		}
		if a, b := licenseStr(licenses[i], "FULLNAME"), licenseStr(licenses[j], "FULLNAME"); a != b {
			return a < b
		}
		return licenseStr(licenses[i], "KEY") < licenseStr(licenses[j], "KEY")
	})
	return licenses
}

func licenseStr(m map[string]any, key string) string {
	s, _ := m[key].(string)
	return s
}
