// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"path/filepath"
	"strconv"
)

// BuildBatteries collects the BATTERIES section from
// <root>/sys/class/power_supply, mirroring Generic/Batteries/SysClass.pm: only
// present batteries with a capacity attribute are reported. Fields: NAME,
// CHEMISTRY, SERIAL, MANUFACTURER, VOLTAGE (mV from voltage_min_design µV),
// CAPACITY (mWh from energy_full_design µWh).
func BuildBatteries(root string) []map[string]any {
	matches, _ := invFS.Glob(filepath.Join(root, "sys/class/power_supply/*"))

	var batteries []map[string]any
	for _, psu := range matches {
		if readSysLine(filepath.Join(psu, "type")) != "Battery" {
			continue
		}
		if !truthy(readSysLine(filepath.Join(psu, "present"))) {
			continue
		}
		if readSysLine(filepath.Join(psu, "capacity")) == "" {
			continue
		}

		battery := map[string]any{}
		setIf(battery, "NAME", readSysLine(filepath.Join(psu, "model_name")))
		setIf(battery, "CHEMISTRY", readSysLine(filepath.Join(psu, "technology")))
		setIf(battery, "SERIAL", readSysLine(filepath.Join(psu, "serial_number")))
		if m := readSysLine(filepath.Join(psu, "manufacturer")); m != "" {
			battery["MANUFACTURER"] = canonicalManufacturer(m)
		}
		// voltage_min_design is in µV -> mV.
		if v := readSysInt(filepath.Join(psu, "voltage_min_design")); v > 0 {
			battery["VOLTAGE"] = v / 1000
		}
		// energy_full_design is in µWh -> mWh.
		if c := readSysInt(filepath.Join(psu, "energy_full_design")); c > 0 {
			battery["CAPACITY"] = c / 1000
		}
		batteries = append(batteries, battery)
	}
	return batteries
}

func readSysInt(path string) int {
	n, err := strconv.Atoi(readSysLine(path))
	if err != nil {
		return 0
	}
	return n
}

func truthy(s string) bool {
	return s != "" && s != "0"
}
