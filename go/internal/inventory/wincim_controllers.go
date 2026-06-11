// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strings"
)

// winControllerClasses are the WMI classes Win32/Controllers.pm scans for PCI
// devices; their objects are concatenated and parsed together.
var winControllerClasses = []string{
	"Win32_FloppyController", "Win32_IDEController", "Win32_SCSIController",
	"Win32_VideoController", "Win32_InfraredDevice", "Win32_USBController",
	"Win32_1394Controller", "Win32_PCMCIAController", "CIM_LogicalDevice",
}

var winControllerProperties = []string{"Name", "Manufacturer", "Caption", "DeviceID"}

var (
	pciVenDevRE    = regexp.MustCompile(`(?i)PCI\\VEN_([0-9a-f]{4})&DEV_([0-9a-f]{4})`)
	winPCISubsysRE = regexp.MustCompile(`(?i)&SUBSYS_([0-9a-f]{4})([0-9a-f]{4})`)
)

// buildWinControllers maps the PnP/controller WMI objects to CONTROLLERS
// entries, mirroring Win32/Controllers.pm: keep only devices whose DeviceID
// carries a PCI VEN/DEV id, parse VENDORID/PRODUCTID/PCISUBSYSTEMID from it, and
// deduplicate by vendor+product. The pci.ids NAME/MANUFACTURER refinement is
// follow-on (the WMI Name/Manufacturer are used as-is).
func buildWinControllers(objects []map[string]any) []map[string]any {
	var controllers []map[string]any
	seen := map[string]bool{}

	for _, o := range objects {
		deviceID := cimString(o, "DeviceID")
		m := pciVenDevRE.FindStringSubmatch(deviceID)
		if m == nil {
			continue // not a PCI device
		}
		vendorID := strings.ToLower(m[1])
		productID := strings.ToLower(m[2])

		key := vendorID + ":" + productID
		if seen[key] {
			continue
		}
		seen[key] = true

		c := map[string]any{
			"VENDORID":  vendorID,
			"PRODUCTID": productID,
		}
		setIf(c, "NAME", cimString(o, "Name"))
		setIf(c, "MANUFACTURER", cimString(o, "Manufacturer"))
		setIf(c, "CAPTION", cimString(o, "Caption"))
		setIf(c, "TYPE", cimString(o, "Caption"))
		if sub := winPCISubsysRE.FindStringSubmatch(deviceID); sub != nil {
			c["PCISUBSYSTEMID"] = strings.ToLower(sub[2] + ":" + sub[1])
		}
		controllers = append(controllers, c)
	}
	return controllers
}
