// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"sort"
)

// buildMacBios maps SPHardwareDataType "Hardware Overview" (plus the ioreg
// IOPlatformExpertDevice attributes) to the BIOS section, mirroring
// MacOS/Bios.pm: SMANUFACTURER/SMODEL/SSN/BVERSION with the documented fallback
// chains. ioreg may be nil.
func buildMacBios(overview, ioreg map[string]any) map[string]any {
	bios := map[string]any{
		"SMANUFACTURER": winFirstNonEmpty(spLeaf(ioreg, "manufacturer"), "Apple Inc"),
	}
	setIf(bios, "SMODEL", winFirstNonEmpty(
		spLeaf(overview, "Model Identifier"), spLeaf(overview, "Machine Model"), spLeaf(ioreg, "model")))
	setIf(bios, "SSN", winFirstNonEmpty(
		spLeaf(overview, "Serial Number"), spLeaf(overview, "Serial Number (system)"),
		spLeaf(ioreg, "IOPlatformSerialNumber")))
	setIf(bios, "BVERSION", spLeaf(overview, "Boot ROM Version"))
	return bios
}

// buildMacCharger maps SPPowerDataType "AC Charger Information" to a
// POWERSUPPLIES entry, mirroring MacOS/Psu.pm _getCharger. Returns nil when no
// charger is present.
func buildMacCharger(powerInfos map[string]any) map[string]any {
	info := spNode(powerInfos, "Power", "AC Charger Information")
	if info == nil {
		return nil
	}
	status := "Not charging"
	if spLeaf(info, "Charging") == "Yes" {
		status = "Charging"
	}
	plugged := spLeaf(info, "Connected")
	if plugged == "" {
		plugged = "No"
	}
	psu := map[string]any{
		"STATUS":  status,
		"PLUGGED": plugged,
	}
	setIf(psu, "SERIALNUMBER", spLeaf(info, "Serial Number"))
	setIf(psu, "NAME", spLeaf(info, "Name"))
	setIf(psu, "MANUFACTURER", spLeaf(info, "Manufacturer"))
	setIf(psu, "POWER_MAX", spLeaf(info, "Wattage (W)"))
	return psu
}

var macResolutionRE = regexp.MustCompile(`(\d+) *x *(\d+)`)

// buildMacVideos maps SPDisplaysDataType "Graphics/Displays" to the VIDEOS
// section, mirroring MacOS/Videos.pm: one entry per graphics card with
// CHIPSET/MEMORY/NAME, the first attached display's RESOLUTION and the
// Bus/Slot PCISLOT.
func buildMacVideos(displayInfos map[string]any) []map[string]any {
	graphics := spNode(displayInfos, "Graphics/Displays")
	if graphics == nil {
		return nil
	}

	names := make([]string, 0, len(graphics))
	for k := range graphics {
		names = append(names, k)
	}
	sort.Strings(names)

	var videos []map[string]any
	for _, name := range names {
		card, ok := graphics[name].(map[string]any)
		if !ok {
			continue
		}
		video := map[string]any{"NAME": name}
		setIf(video, "CHIPSET", spLeaf(card, "Chipset Model"))
		if mem := canonicalSizeMB(winFirstNonEmpty(spLeaf(card, "VRAM (Total)"), spLeaf(card, "VRAM (Dynamic, Max)"))); mem > 0 {
			video["MEMORY"] = mem
		}
		if res := macFirstResolution(card); res != "" {
			video["RESOLUTION"] = res
		}
		if pcislot := winFirstNonEmpty(spLeaf(card, "Bus"), spLeaf(card, "Slot")); pcislot != "" {
			video["PCISLOT"] = pcislot
		}
		videos = append(videos, video)
	}
	return videos
}

// macFirstResolution returns the first attached display's "WxH" resolution.
func macFirstResolution(card map[string]any) string {
	displays := spNode(card, "Displays")
	if displays == nil {
		return ""
	}
	names := make([]string, 0, len(displays))
	for k := range displays {
		names = append(names, k)
	}
	sort.Strings(names)
	for _, dn := range names {
		if dn == "Display Connector" || dn == "Display" {
			continue
		}
		display, ok := displays[dn].(map[string]any)
		if !ok {
			continue
		}
		if m := macResolutionRE.FindStringSubmatch(spLeaf(display, "Resolution")); m != nil {
			return m[1] + "x" + m[2]
		}
	}
	return ""
}
