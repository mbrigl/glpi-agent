// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strconv"
	"strings"
	"testing"
)

// hexBytes parses a "a4,00,03,.." comma-separated hex byte string (the form used
// by the upstream license.t vectors).
func hexBytes(t *testing.T, s string) []byte {
	t.Helper()
	parts := strings.Split(s, ",")
	b := make([]byte, len(parts))
	for i, p := range parts {
		n, err := strconv.ParseUint(strings.TrimSpace(p), 16, 8)
		if err != nil {
			t.Fatalf("bad hex byte %q: %v", p, err)
		}
		b[i] = byte(n)
	}
	return b
}

// TestDecodeMicrosoftKey pins the product-key decoder against the real binary
// DigitalProductID vectors from the upstream t/agent/tools/license.t.
func TestDecodeMicrosoftKey(t *testing.T) {
	const win8 = "a4,00,00,00,03,00,00,00,30,30,31,38,30,2d,31,30,35,33,39,2d,35,32,38,34,30,2d,41,41,4f,45,4d,00,09,07,00,00,58,31,38,2d,31,35,35,38,30,00,00,00,00,00,00,00,09,07,80,14,74,33,14,aa,32,e4,d5,11,25,15,08,00,00,00,00,00,3a,05,bb,51,2f,01,29,97,02,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,b6,7d,17,ed"
	const win7null = "a4,00,00,00,03,00,00,00,35,35,30,34,31,2d,30,32,39,2d,30,30,34,37,38,39,37,2d,38,36,36,32,34,00,ac,00,00,00,58,31,35,2d,33,39,30,38,31,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,39,69,0a,52,80,bd,80,2c,03,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,5d,8a,cd,c9"

	if got := decodeMicrosoftKey(hexBytes(t, win8)); got != "NK2HF-3VG6G-X3YMF-JFT99-HCBRC" {
		t.Errorf("win8 key = %q, want NK2HF-3VG6G-X3YMF-JFT99-HCBRC", got)
	}
	// A null (all-zero) key range decodes to "".
	if got := decodeMicrosoftKey(hexBytes(t, win7null)); got != "" {
		t.Errorf("win7 null key = %q, want empty", got)
	}
}

// TestBuildOfficeLicense checks the _getOfficeLicense field mapping incl. the
// COMPONENTS collection and the TrialType decode.
func TestBuildOfficeLicense(t *testing.T) {
	values := map[string]string{
		"ProductID":               "12345-678",
		"SPLevel":                 "1",
		"OEM":                     "1",
		"ProductName":             "Microsoft Office Professional 2016",
		"ProductNameNonQualified": "Office Professional 2016",
		"TrialType":               "something-30",
		"WordNameVersion":         "16.0",
		"ExcelNameVersion":        "16.0",
	}
	lic := buildOfficeLicense(values, nil) // no DigitalProductID -> no KEY
	if lic["FULLNAME"] != "Microsoft Office Professional 2016" || lic["NAME"] != "Office Professional 2016" {
		t.Errorf("office names = %v", lic)
	}
	if lic["UPDATE"] != "1" || lic["PRODUCTID"] != "12345-678" || lic["TRIAL"] != 30 {
		t.Errorf("office fields = %v", lic)
	}
	if lic["COMPONENTS"] != "Excel/Word" {
		t.Errorf("components = %v, want Excel/Word", lic["COMPONENTS"])
	}
	if _, ok := lic["KEY"]; ok {
		t.Errorf("unexpected KEY without DigitalProductID")
	}
}

// TestMergeWinLicensesOfficeStub checks that a WMI license matching an Office
// PRODUCTCODE stub is promoted (inheriting FULLNAME) and re-keyed.
func TestMergeWinLicensesOfficeStub(t *testing.T) {
	seen := map[string]map[string]any{
		// Office registration stub for uuid "uuid-1" pointing at a product code.
		"uuid-1": {"PRODUCTCODE": "pcode-1", "FULLNAME": "Office Professional 2016"},
	}
	wmi := []map[string]any{
		{"ID": "UUID-1", "PartialProductKey": "ABCDE", "LicenseStatus": float64(1), "Name": "Office16"},
	}
	lic := mergeWinLicenses(seen, wmi)
	if len(lic) != 1 {
		t.Fatalf("got %d licenses, want 1", len(lic))
	}
	if lic[0]["FULLNAME"] != "Office Professional 2016" || lic[0]["NAME"] != "Office16" ||
		lic[0]["KEY"] != "XXXXX-XXXXX-XXXXX-XXXXX-ABCDE" {
		t.Errorf("promoted license = %v", lic[0])
	}
	// The original uuid key was retargeted to the product code.
	if _, ok := seen["uuid-1"]; ok {
		t.Errorf("uuid-1 should have been re-keyed to pcode-1")
	}
}

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
		// Duplicate ID of Office -> merged: the new KEY/NAME win, but FULLNAME is
		// inherited from the already-seen entry (upstream merge rule).
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
	// FULLNAME inherited from the first-seen entry; OEM from the merged (RETAIL) one.
	if office["OEM"] != 0 || office["FULLNAME"] != "Office 16, RETAIL channel" {
		t.Errorf("office OEM/FULLNAME = %v / %v", office["OEM"], office["FULLNAME"])
	}

	visio := lic[1]
	if visio["NAME"] != "Visio" || visio["OEM"] != 1 ||
		visio["KEY"] != "FULL1-FULL2-FULL3-FULL4-FULL5" || visio["PRODUCTID"] != "appid-visio" {
		t.Errorf("visio = %v", visio)
	}
}
