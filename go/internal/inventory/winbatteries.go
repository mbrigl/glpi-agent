// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/xml"
	"regexp"
	"strconv"
	"strings"
)

// powercfgReport models the relevant part of a `powercfg /batteryreport /xml`
// document (BatteryReport > Batteries > Battery). Element names match by local
// name regardless of the report's XML namespace.
type powercfgReport struct {
	Batteries struct {
		Battery []powercfgBattery `xml:"Battery"`
	} `xml:"Batteries"`
}

type powercfgBattery struct {
	ID                 string `xml:"Id"`
	Manufacturer       string `xml:"Manufacturer"`
	SerialNumber       string `xml:"SerialNumber"`
	Chemistry          string `xml:"Chemistry"`
	DesignCapacity     string `xml:"DesignCapacity"`
	FullChargeCapacity string `xml:"FullChargeCapacity"`
}

// parsePowercfgBatteries maps a powercfg battery report to BATTERIES entries,
// mirroring Win32/Batteries.pm _getBatteriesFromPowercfg: NAME(Id),
// MANUFACTURER, CHEMISTRY, sanitised SERIAL, CAPACITY (DesignCapacity, mWh) and
// REAL_CAPACITY (FullChargeCapacity, mWh). Document order is preserved.
func parsePowercfgBatteries(data []byte) []map[string]any {
	var report powercfgReport
	if err := xml.Unmarshal(data, &report); err != nil {
		return nil
	}

	var batteries []map[string]any
	for _, b := range report.Batteries.Battery {
		battery := map[string]any{}
		setIf(battery, "NAME", strings.TrimSpace(b.ID))
		setIf(battery, "MANUFACTURER", strings.TrimSpace(b.Manufacturer))
		setIf(battery, "CHEMISTRY", strings.TrimSpace(b.Chemistry))
		setIf(battery, "SERIAL", sanitizeBatterySerial(b.SerialNumber))
		if c := canonicalCapacityMWh(b.DesignCapacity); c > 0 {
			battery["CAPACITY"] = c
		}
		if c := canonicalCapacityMWh(b.FullChargeCapacity); c > 0 {
			battery["REAL_CAPACITY"] = c
		}
		batteries = append(batteries, battery)
	}
	return batteries
}

// canonicalCapacityMWh parses a bare mWh capacity figure (powercfg emits a plain
// integer); 0 means absent/invalid. Mirrors getCanonicalCapacity("<n> mWh").
func canonicalCapacityMWh(s string) int {
	n, err := strconv.Atoi(strings.TrimSpace(s))
	if err != nil || n < 0 {
		return 0
	}
	return n
}

var (
	batterySerialZerosRE = regexp.MustCompile(`^0+$`)
	batterySerialHexRE   = regexp.MustCompile(`^[0-9A-Fa-f]+$`)
	batterySerialAFRE    = regexp.MustCompile(`[a-fA-F]`)
)

// sanitizeBatterySerial mirrors Tools/Batteries.pm sanitizeBatterySerial: an
// empty/zeros-only serial becomes "0"; a value with non-hex characters is just
// trimmed; an all-hex value recognised as hexadecimal (containing a-f or a
// leading zero) is converted to its decimal string, otherwise returned as-is.
func sanitizeBatterySerial(serial string) string {
	if serial == "" || batterySerialZerosRE.MatchString(serial) {
		return "0"
	}
	if !batterySerialHexRE.MatchString(serial) {
		return strings.TrimSpace(serial)
	}
	value := serial
	if batterySerialAFRE.MatchString(serial) || strings.HasPrefix(serial, "0") {
		value = "0x" + serial
	}
	if n, ok := hex2dec(value); ok {
		return strconv.Itoa(n)
	}
	return strings.TrimSpace(serial)
}
