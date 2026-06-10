// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strconv"
	"strings"
)

// CIM property lists for the Windows bios + hardware categories.
var (
	winCSProperties = []string{
		"Manufacturer", "Model", "Name", "DNSHostName", "Domain", "Workgroup",
		"PrimaryOwnerName", "TotalPhysicalMemory",
	}
	winBiosProperties      = []string{"SerialNumber", "Version", "Manufacturer", "SMBIOSBIOSVersion", "BIOSVersion", "ReleaseDate"}
	winEnclosureProperties = []string{"SerialNumber", "SMBIOSAssetTag"}
	winBaseBoardProperties = []string{"SerialNumber", "Product", "Manufacturer"}
	winCSProductProperties = []string{"UUID"}
)

// winFirstNonEmpty returns the first non-empty string.
func winFirstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if v != "" {
			return v
		}
	}
	return ""
}

// buildWinBios maps Win32_Bios / Win32_ComputerSystem / Win32_SystemEnclosure /
// Win32_BaseBoard CIM objects to the BIOS section, mirroring Win32/Bios.pm
// (including the SSN fallback chain bios -> enclosure -> baseboard).
func buildWinBios(bios, cs, enclosure, baseboard map[string]any) map[string]any {
	b := map[string]any{}

	setSSN := func(s string) {
		if s == "" {
			return
		}
		if _, ok := b["SSN"]; !ok {
			b["SSN"] = s
		}
	}

	// Win32_Bios.
	if s := cimString(bios, "SerialNumber"); s != "" {
		b["BIOSSERIAL"] = s
		b["SSN"] = s
	}
	setIf(b, "BMANUFACTURER", cimString(bios, "Manufacturer"))
	setIf(b, "BVERSION", winFirstNonEmpty(cimString(bios, "SMBIOSBIOSVersion"), cimString(bios, "BIOSVersion"), cimString(bios, "Version")))
	setIf(b, "BDATE", dateFromIntString(cimString(bios, "ReleaseDate")))

	// Win32_ComputerSystem.
	setIf(b, "SMANUFACTURER", cimString(cs, "Manufacturer"))
	setIf(b, "SMODEL", cimString(cs, "Model"))

	// Win32_SystemEnclosure.
	if s := cimString(enclosure, "SerialNumber"); s != "" {
		b["ENCLOSURESERIAL"] = s
		setSSN(s)
	}
	setIf(b, "ASSETTAG", cimString(enclosure, "SMBIOSAssetTag"))

	// Win32_BaseBoard.
	if s := cimString(baseboard, "SerialNumber"); s != "" {
		b["MSN"] = s
		setSSN(s)
	}
	setIf(b, "MMODEL", cimString(baseboard, "Product"))
	if m := cimString(baseboard, "Manufacturer"); m != "" {
		b["MMANUFACTURER"] = m
		if _, ok := b["SMANUFACTURER"]; !ok {
			b["SMANUFACTURER"] = m
		}
	}

	// Trim trailing whitespace and drop placeholder/invalid values.
	for k, v := range b {
		s, ok := v.(string)
		if !ok {
			continue
		}
		s = strings.TrimRight(s, " ")
		if isInvalidBiosValue(s) {
			delete(b, k)
		} else {
			b[k] = s
		}
	}
	return b
}

// dateFromIntString turns a WMI "YYYYMMDD…" date into "MM/DD/YYYY"
// (Win32/Bios.pm::_dateFromIntString); other input is returned unchanged.
func dateFromIntString(s string) string {
	if m := regexp.MustCompile(`^(\d{4})(\d{2})(\d{2})`).FindStringSubmatch(s); m != nil {
		return m[2] + "/" + m[3] + "/" + m[1]
	}
	return s
}

var uuidPlaceholderRE = regexp.MustCompile(`^[0-]+$`)

// buildWinHardware maps Win32_OperatingSystem / Win32_ComputerSystem /
// Win32_ComputerSystemProduct to the HARDWARE fields, mirroring Win32/Hardware.pm.
// The registry-derived WINPRODKEY and DESCRIPTION are follow-on.
func buildWinHardware(osObj, cs, csProduct map[string]any) map[string]any {
	h := map[string]any{}

	setIf(h, "NAME", winFirstNonEmpty(cimString(cs, "DNSHostName"), cimString(cs, "Name")))
	if uuid := cimString(csProduct, "UUID"); uuid != "" && !uuidPlaceholderRE.MatchString(uuid) {
		h["UUID"] = uuid
	}
	setIf(h, "WINLANG", cimString(osObj, "OSLanguage"))
	setIf(h, "WINPRODID", cimString(osObj, "SerialNumber"))
	setIf(h, "WINCOMPANY", cimString(osObj, "Organization"))
	setIf(h, "WINOWNER", winFirstNonEmpty(cimString(osObj, "RegisteredUser"), cimString(cs, "PrimaryOwnerName")))
	setIf(h, "WORKGROUP", winFirstNonEmpty(cimString(cs, "Domain"), cimString(cs, "Workgroup")))
	if mem := cimBytesToMB(cs, "TotalPhysicalMemory"); mem > 0 {
		h["MEMORY"] = mem
	}
	if swap := cimBytesToMB(osObj, "TotalSwapSpaceSize"); swap > 0 {
		h["SWAP"] = swap
	}
	return h
}

// cimBytesToMB reads a byte count CIM property (a big integer, possibly rendered
// as a string by ConvertTo-Json) and returns it in mebibytes, or 0.
func cimBytesToMB(obj map[string]any, key string) int {
	n, err := strconv.ParseInt(cimString(obj, key), 10, 64)
	if err != nil || n <= 0 {
		return 0
	}
	return int(n / (1024 * 1024))
}
