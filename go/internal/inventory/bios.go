// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strings"
)

// dmiToBios maps a /sys/class/dmi/id field name to its BIOS section key. The
// fields mirror Generic/Dmidecode/Bios.pm (sysfs DMI exposes the same values
// dmidecode prints).
var dmiToBios = []struct{ dmi, field string }{
	{"bios_vendor", "BMANUFACTURER"},
	{"bios_version", "BVERSION"},
	{"bios_date", "BDATE"},
	{"sys_vendor", "SMANUFACTURER"},
	{"product_name", "SMODEL"},
	{"product_sku", "SKUNUMBER"},
	{"board_vendor", "MMANUFACTURER"},
	{"board_name", "MMODEL"},
	{"board_serial", "MSN"},
	{"chassis_asset_tag", "ASSETTAG"},
}

// ParseDMI builds the BIOS section from /sys/class/dmi/id field values,
// mirroring Generic/Dmidecode/Bios.pm: invalid placeholder values are dropped,
// SSN falls back from system to chassis serial, and the ASSETTAG/SKUNUMBER
// "Tag#"/"SKU#" defaults are removed.
func ParseDMI(dmi map[string]string) map[string]any {
	bios := map[string]any{}
	put := func(field, value string) {
		value = strings.TrimSpace(value)
		if value != "" && !isInvalidBiosValue(value) {
			bios[field] = value
		}
	}

	for _, m := range dmiToBios {
		put(m.field, dmi[m.dmi])
	}

	// SSN: system serial, falling back to chassis serial (Bios.pm).
	ssn := strings.TrimSpace(dmi["product_serial"])
	if ssn == "" || isInvalidBiosValue(ssn) {
		ssn = strings.TrimSpace(dmi["chassis_serial"])
	}
	if ssn != "" && !isInvalidBiosValue(ssn) {
		bios["SSN"] = ssn
	}

	// Drop default-content ASSETTAG/SKUNUMBER ending in a sharp marker.
	if v, ok := bios["ASSETTAG"].(string); ok && strings.HasSuffix(v, "Tag#") {
		delete(bios, "ASSETTAG")
	}
	if v, ok := bios["SKUNUMBER"].(string); ok && strings.HasSuffix(v, "SKU#") {
		delete(bios, "SKUNUMBER")
	}
	return bios
}

// invalidBiosValue mirrors the placeholder set in
// GLPI::Agent::Tools::Generic::isInvalidBiosValue.
var invalidBiosValue = regexp.MustCompile(`(?i)^(?:` +
	`N/A|None|Unknown|Not\s*Specified|Not\s*Present|Not\s*Available|Not\s*Installed|` +
	`Default\s*string|System\s*Product\s*Name|System\s*manufacturer|System\s*Serial\s*Number|` +
	`System\s*Version|Chassis\s*Serial\s*Number|Chassis\s*manufacturer?|Chassis\s*Version|` +
	`No\s*Asset\s*Tag|<BAD\s*INDEX>|(?:<OUT\s*OF\s*SPEC>){1,2}|` +
	`\s*To\s*Be\s*Filled\s*By\s*O\.E\.M\.` +
	`)$`)

func isInvalidBiosValue(v string) bool {
	return invalidBiosValue.MatchString(strings.TrimSpace(v))
}
