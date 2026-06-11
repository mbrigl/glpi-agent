// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"fmt"
	"regexp"
	"sort"
	"strconv"
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
var winTrialTypeRE = regexp.MustCompile(`(\d+)$`)
var winNameVersionRE = regexp.MustCompile(`^(\w+)NameVersion$`)

// winKeyLetters are the 24 characters a Microsoft product key is encoded with
// (Tools/License.pm decodeMicrosoftKey).
const winKeyLetters = "BCDFGHJKMPQRTVWXY2346789"

// decodeMicrosoftKey decodes a binary DigitalProductID into the
// "XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" product key, mirroring
// Tools/License.pm decodeMicrosoftKey (inspired by http://poshcode.org/4363).
// It selects bytes 808..822 for new-style keys (Office 2010+) or 52..66 for the
// old style, handles the Windows 8 / Office 2013 "N" key flag, and returns "" for
// an absent or all-zero key.
func decodeMicrosoftKey(raw []byte) string {
	firstByte := 52
	if len(raw) > 808 {
		firstByte = 808
	}
	lastByte := firstByte + 14
	if lastByte >= len(raw) {
		return ""
	}

	bytes := make([]int, 15)
	for i := range bytes {
		bytes[i] = int(raw[firstByte+i])
	}

	// Windows 8 / Office 2013 keys carry an "N" flag in bit 3 of the last byte.
	containsN := (bytes[14] >> 3) & 1
	bytes[14] &= 0xF7

	allZero := true
	for _, b := range bytes {
		if b != 0 {
			allZero = false
			break
		}
	}
	if allZero {
		return ""
	}

	const charsLength = 25
	chars := make([]byte, charsLength)
	for i := charsLength - 1; i >= 0; i-- {
		index := 0
		for j := len(bytes) - 1; j >= 0; j-- {
			value := (index << 8) | bytes[j]
			bytes[j] = value / len(winKeyLetters)
			index = value % len(winKeyLetters)
		}
		chars[i] = winKeyLetters[index]
	}

	if containsN != 0 {
		first := chars[0]
		rest := chars[1:]
		idx := 0
		for i := 0; i < len(winKeyLetters); i++ {
			if winKeyLetters[i] == first {
				idx = i
				break
			}
		}
		out := make([]byte, 0, charsLength)
		out = append(out, rest[:idx]...)
		out = append(out, 'N')
		out = append(out, rest[idx:]...)
		chars = out
	}

	var sb strings.Builder
	for i, c := range chars {
		if i > 0 && i%5 == 0 {
			sb.WriteByte('-')
		}
		sb.WriteByte(c)
	}
	return sb.String()
}

// buildOfficeLicense maps an Office registration registry entry (string values +
// the raw DigitalProductID) to a license, mirroring Win32/License.pm
// _getOfficeLicense: KEY (decoded), PRODUCTID, UPDATE (SPLevel), OEM, FULLNAME
// (ProductName||ConvertToEdition), NAME (ProductNameNonQualified||
// ProductNameVersion), TRIAL (trailing digits of TrialType) and COMPONENTS
// (sorted "<App>" of every "<App>NameVersion" value).
func buildOfficeLicense(values map[string]string, digitalProductID []byte) map[string]any {
	license := map[string]any{}
	if k := decodeMicrosoftKey(digitalProductID); k != "" {
		license["KEY"] = k
	}
	setIf(license, "PRODUCTID", values["ProductID"])
	setIf(license, "UPDATE", values["SPLevel"])
	setIf(license, "OEM", values["OEM"])
	setIf(license, "FULLNAME", winFirstNonEmpty(values["ProductName"], values["ConvertToEdition"]))
	setIf(license, "NAME", winFirstNonEmpty(values["ProductNameNonQualified"], values["ProductNameVersion"]))

	if m := winTrialTypeRE.FindStringSubmatch(values["TrialType"]); m != nil {
		if n, err := strconv.Atoi(m[1]); err == nil {
			license["TRIAL"] = n
		}
	}

	var components []string
	for name := range values {
		if m := winNameVersionRE.FindStringSubmatch(name); m != nil {
			components = append(components, m[1])
		}
	}
	if len(components) > 0 {
		sort.Strings(components)
		license["COMPONENTS"] = strings.Join(components, "/")
	}
	return license
}

// buildWinLicenses maps SoftwareLicensingProduct (root/CIMV2) objects to
// LICENSEINFOS, mirroring Win32/License.pm _scanWmiSoftwareLicensingProducts +
// _getWmiLicense: only licensed products with a partial product key are kept,
// the OS license is skipped (it comes from the OS module), entries are
// deduplicated by lc(ID) and sorted by NAME/FULLNAME/KEY. The Office-registry
// (DigitalProductID decode) and Adobe cache.db sources are follow-on.
func buildWinLicenses(objects []map[string]any) []map[string]any {
	return mergeWinLicenses(map[string]map[string]any{}, objects)
}

// mergeWinLicenses folds the SoftwareLicensingProduct WMI objects into the
// products already seen from the Office registry scan and returns the sorted
// LICENSEINFOS, mirroring Win32/License.pm _scanWmiSoftwareLicensingProducts +
// _getSeenProducts. A WMI entry whose ID matches an Office stub inherits the
// stub's FULLNAME/TRIAL and is re-keyed to the stub's PRODUCTCODE; when the
// registry already carries the matching key (same last 5 chars) the WMI entry is
// dropped so the registry wins. Only products with a KEY are emitted, sorted by
// NAME/FULLNAME/KEY.
func mergeWinLicenses(seen map[string]map[string]any, objects []map[string]any) []map[string]any {
	for _, o := range objects {
		if cimString(o, "PartialProductKey") == "" || cimInt(o, "LicenseStatus") == 0 {
			continue
		}
		if winLicenseOSRE.MatchString(cimString(o, "Description")) {
			continue
		}
		id := strings.ToLower(cimString(o, "ID"))
		if id == "" {
			continue
		}

		wmiLicense := getWmiLicense(o)
		existing, ok := seen[id]
		if !ok {
			seen[id] = wmiLicense
			continue
		}

		if fn := licenseStr(existing, "FULLNAME"); fn != "" {
			wmiLicense["FULLNAME"] = fn
		}
		if tr, ok := existing["TRIAL"]; ok {
			wmiLicense["TRIAL"] = tr
		}

		toDelete := id
		if pc := licenseStr(existing, "PRODUCTCODE"); pc != "" {
			id = pc
			if target, ok := seen[id]; ok {
				if k := licenseStr(target, "KEY"); k != "" {
					wmiKey := licenseStr(wmiLicense, "KEY")
					if len(wmiKey) >= 5 && strings.HasSuffix(k, wmiKey[len(wmiKey)-5:]) {
						// Registry already has the matching key; keep it.
						continue
					}
				}
			}
		}
		delete(seen, toDelete)
		seen[id] = wmiLicense
	}

	var licenses []map[string]any
	for _, l := range seen {
		if licenseStr(l, "KEY") == "" {
			continue
		}
		licenses = append(licenses, l)
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

// getWmiLicense maps one SoftwareLicensingProduct object to a license, mirroring
// Win32/License.pm _getWmiLicense (5-char partial key expanded, PRODUCTID
// fallback chain, OEM from the channel).
func getWmiLicense(o map[string]any) map[string]any {
	key := cimString(o, "PartialProductKey")
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
	return license
}

func licenseStr(m map[string]any, key string) string {
	s, _ := m[key].(string)
	return s
}
