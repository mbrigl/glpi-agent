// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinLicenses checks the SoftwareLicensingProduct -> LICENSEINFOS
// mapping: unlicensed / keyless / OS products are skipped, the partial key is
// expanded, OEM/PRODUCTID are derived, and entries are sorted by NAME.
func TestBuildWinLicenses(t *testing.T) {
	objs := []map[string]any{
		// Office: licensed, OEM channel, 5-char partial key expanded.
		{
			"Name": "Office16", "Description": "Office 16, RETAIL channel",
			"LicenseStatus": float64(1), "PartialProductKey": "ABCDE",
			"ID": "id-office", "ProductKeyChannel": "Retail",
			"ProductKeyID2": "PKID2-office",
		},
		// Windows: licensed but OS license -> skipped.
		{
			"Name": "Windows", "Description": "Windows Operating System - Professional",
			"LicenseStatus": float64(1), "PartialProductKey": "WWWWW", "ID": "id-os",
		},
		// Unlicensed -> skipped.
		{"Name": "Unlic", "LicenseStatus": float64(0), "PartialProductKey": "XXXXX", "ID": "id-unlic"},
		// No partial key -> skipped.
		{"Name": "NoKey", "LicenseStatus": float64(1), "ID": "id-nokey"},
		// Visio: OEM channel, full key kept as-is, ApplicationID fallback.
		{
			"Name": "Visio", "Description": "Visio Pro",
			"LicenseStatus": float64(1), "PartialProductKey": "FULL1-FULL2-FULL3-FULL4-FULL5",
			"ID": "id-visio", "ProductKeyChannel": "OEM:DM", "ApplicationID": "appid-visio",
		},
		// Duplicate ID of Office -> deduped (last wins).
		{
			"Name": "Office16", "Description": "Office 16 dup",
			"LicenseStatus": float64(1), "PartialProductKey": "ZZZZZ", "ID": "ID-OFFICE",
		},
	}

	lic := buildWinLicenses(objs)
	if len(lic) != 2 {
		t.Fatalf("got %d licenses, want 2 (skips + dedupe)", len(lic))
	}

	// Sorted by NAME: "Office16" before "Visio".
	office := lic[0]
	if office["NAME"] != "Office16" || office["KEY"] != "XXXXX-XXXXX-XXXXX-XXXXX-ZZZZZ" {
		t.Errorf("office = %v", office)
	}
	if office["OEM"] != 0 || office["FULLNAME"] != "Office 16 dup" {
		t.Errorf("office OEM/FULLNAME = %v / %v", office["OEM"], office["FULLNAME"])
	}

	visio := lic[1]
	if visio["NAME"] != "Visio" || visio["OEM"] != 1 ||
		visio["KEY"] != "FULL1-FULL2-FULL3-FULL4-FULL5" || visio["PRODUCTID"] != "appid-visio" {
		t.Errorf("visio = %v", visio)
	}
}
