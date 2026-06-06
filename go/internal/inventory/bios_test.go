// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestParseDMI(t *testing.T) {
	dmi := map[string]string{
		"bios_vendor":       "Dell Inc.",
		"bios_version":      "3.23.1",
		"bios_date":         "01/26/2026",
		"sys_vendor":        "Dell Inc.",
		"product_name":      "Precision 3460",
		"product_serial":    "ABCD123",
		"board_vendor":      "Dell Inc.",
		"board_name":        "08PFGW",
		"chassis_asset_tag": "To Be Filled By O.E.M.", // invalid -> dropped
		"chassis_serial":    "Default string",         // invalid
	}
	bios := ParseDMI(dmi)

	if bios["BMANUFACTURER"] != "Dell Inc." || bios["BVERSION"] != "3.23.1" || bios["BDATE"] != "01/26/2026" {
		t.Errorf("BIOS firmware fields wrong: %v", bios)
	}
	if bios["SMODEL"] != "Precision 3460" || bios["MMODEL"] != "08PFGW" {
		t.Errorf("model fields wrong: %v", bios)
	}
	if bios["SSN"] != "ABCD123" {
		t.Errorf("SSN = %v, want ABCD123", bios["SSN"])
	}
	if _, present := bios["ASSETTAG"]; present {
		t.Errorf("ASSETTAG should be dropped as an invalid placeholder: %v", bios["ASSETTAG"])
	}
}

func TestParseDMISSNFallback(t *testing.T) {
	// System serial invalid -> fall back to chassis serial.
	bios := ParseDMI(map[string]string{
		"product_serial": "System Serial Number",
		"chassis_serial": "CHASSIS99",
	})
	if bios["SSN"] != "CHASSIS99" {
		t.Errorf("SSN = %v, want chassis fallback CHASSIS99", bios["SSN"])
	}
}

func TestIsInvalidBiosValue(t *testing.T) {
	invalid := []string{"N/A", "None", "Unknown", "Not Specified", "Default string",
		"To Be Filled By O.E.M.", "System Serial Number", "<OUT OF SPEC>"}
	for _, v := range invalid {
		if !isInvalidBiosValue(v) {
			t.Errorf("%q should be invalid", v)
		}
	}
	for _, v := range []string{"ABCD123", "Dell Inc.", "Precision 3460"} {
		if isInvalidBiosValue(v) {
			t.Errorf("%q should be valid", v)
		}
	}
}
