// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "strconv"

var (
	winKeyboardProperties = []string{"Name", "Caption", "Manufacturer", "Description", "Layout"}
	winPointingProperties = []string{"Name", "Caption", "Manufacturer", "Description", "PointingType", "DeviceInterface"}
	winModemProperties    = []string{"Name", "DeviceType", "Model", "Description"}
	winEnclosureChassis   = []string{"ChassisTypes"}

	// Win32_PointingDevice.DeviceInterface -> INTERFACE (Win32/Inputs.pm).
	winMouseInterface = map[int]string{
		1: "Other", 2: "Unknown", 3: "Serial", 4: "PS/2", 5: "Infrared",
		6: "HP-HIL", 7: "Bus Mouse", 8: "ADB (Apple Desktop Bus)",
		160: "Bus Mouse DB-9", 161: "Bus Mouse Micro-DIN", 162: "USB",
	}

	// Win32_SystemEnclosure.ChassisTypes[0] -> CHASSIS_TYPE (Win32/Chassis.pm).
	winChassisType = []string{
		"Unknown", "Other", "Unknown", "Desktop", "Low Profile Desktop",
		"Pizza Box", "Mini Tower", "Tower", "Portable", "Laptop", "Notebook",
		"Hand Held", "Docking Station", "All in One", "Sub Notebook",
		"Space-Saving", "Lunch Box", "Main System Chassis", "Expansion Chassis",
		"SubChassis", "Bus Expansion Chassis", "Peripheral Chassis",
		"Storage Chassis", "Rack Mount Chassis", "Sealed-Case PC",
	}
)

// buildWinInputs maps Win32_Keyboard and Win32_PointingDevice to INPUTS,
// mirroring Win32/Inputs.pm: keyboards carry LAYOUT, pointing devices carry
// POINTINGTYPE and the decoded INTERFACE; entries are deduplicated by NAME across
// both classes.
func buildWinInputs(keyboards, pointing []map[string]any) []map[string]any {
	var out []map[string]any
	seen := map[string]bool{}

	add := func(in map[string]any) {
		name := in["NAME"].(string)
		if name == "" || seen[name] {
			return
		}
		seen[name] = true
		out = append(out, in)
	}

	for _, o := range keyboards {
		in := map[string]any{"NAME": cimString(o, "Name")}
		setIf(in, "CAPTION", cimString(o, "Caption"))
		setIf(in, "MANUFACTURER", cimString(o, "Manufacturer"))
		setIf(in, "DESCRIPTION", cimString(o, "Description"))
		setIf(in, "LAYOUT", cimString(o, "Layout"))
		add(in)
	}
	for _, o := range pointing {
		in := map[string]any{"NAME": cimString(o, "Name")}
		setIf(in, "CAPTION", cimString(o, "Caption"))
		setIf(in, "MANUFACTURER", cimString(o, "Manufacturer"))
		setIf(in, "DESCRIPTION", cimString(o, "Description"))
		setIf(in, "POINTINGTYPE", cimString(o, "PointingType"))
		if iface, ok := winMouseInterface[cimInt(o, "DeviceInterface")]; ok {
			in["INTERFACE"] = iface
		}
		add(in)
	}
	return out
}

// buildWinModems maps Win32_POTSModem to MODEMS, mirroring Win32/Modems.pm.
func buildWinModems(objects []map[string]any) []map[string]any {
	var out []map[string]any
	for _, o := range objects {
		name := cimString(o, "Name")
		if name == "" {
			continue
		}
		m := map[string]any{"NAME": name}
		setIf(m, "TYPE", cimString(o, "DeviceType"))
		setIf(m, "MODEL", cimString(o, "Model"))
		setIf(m, "DESCRIPTION", cimString(o, "Description"))
		out = append(out, m)
	}
	return out
}

// winChassis returns the CHASSIS_TYPE for the hardware section from a
// Win32_SystemEnclosure object, mirroring Win32/Chassis.pm: the first
// ChassisTypes entry decoded against the SMBIOS chassis table.
func winChassis(enclosure map[string]any) string {
	if enclosure == nil {
		return ""
	}
	idx, err := strconv.Atoi(cimFirstOfArray(enclosure, "ChassisTypes"))
	if err != nil {
		return ""
	}
	return enumAt(winChassisType, idx)
}
