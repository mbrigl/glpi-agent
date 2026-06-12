// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "sort"

// buildMacSounds maps the SPAudioDataType "Audio (Built In)" node to the SOUNDS
// section, mirroring MacOS/Sound.pm: each sub-device name becomes a SOUNDS entry
// with NAME/MANUFACTURER/DESCRIPTION all set to that name.
func buildMacSounds(audioInfos map[string]any) []map[string]any {
	node := spNode(audioInfos, "Audio (Built In)")
	if node == nil {
		return nil
	}
	names := make([]string, 0, len(node))
	for k := range node {
		names = append(names, k)
	}
	sort.Strings(names)

	var sounds []map[string]any
	for _, name := range names {
		sounds = append(sounds, map[string]any{
			"NAME":         name,
			"MANUFACTURER": name,
			"DESCRIPTION":  name,
		})
	}
	return sounds
}

// macFirewallStatus maps the application-firewall service state + globalstate to
// the FIREWALL STATUS, mirroring MacOS/Firewall.pm: "on" only when the alf
// service runs and globalstate is "1".
func macFirewallStatus(serviceRunning bool, globalstate string) string {
	if serviceRunning && globalstate == "1" {
		return "on"
	}
	return "off"
}

// macHostname returns the SPSoftwareDataType "Computer Name" for the hardware
// NAME, mirroring MacOS/Hostname.pm.
func macHostname(softwareInfos map[string]any) string {
	return spString(softwareInfos, "Software", "System Software Overview", "Computer Name")
}

// buildMacBattery maps the SPPowerDataType "Battery Information" to a BATTERIES
// entry, mirroring MacOS/Batteries.pm: SERIAL/NAME/MANUFACTURER from Model
// Information, CAPACITY from Charge Information, VOLTAGE from Battery Information.
// Returns nil when there is no battery.
func buildMacBattery(powerInfos map[string]any) map[string]any {
	info := spNode(powerInfos, "Power", "Battery Information")
	if info == nil {
		return nil
	}
	battery := map[string]any{}
	model := spNode(info, "Model Information")
	setIf(battery, "SERIAL", spLeaf(model, "Serial Number"))
	setIf(battery, "NAME", spLeaf(model, "Device Name"))
	setIf(battery, "MANUFACTURER", spLeaf(model, "Manufacturer"))
	if charge := spNode(info, "Charge Information"); charge != nil {
		setIf(battery, "CAPACITY", spLeaf(charge, "Full Charge Capacity (mAh)"))
	}
	setIf(battery, "VOLTAGE", spLeaf(info, "Voltage (mV)"))
	if len(battery) == 0 {
		return nil
	}
	return battery
}
