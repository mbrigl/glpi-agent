// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strings"
)

// winUSBProperties are the CIM_LogicalDevice properties for the usb inventory
// (Win32/USB.pm _getDevicesFromWMI).
var winUSBProperties = []string{"Caption", "DeviceID", "Name"}

var (
	// USB device-instance id: "USB\VID_xxxx&PID_xxxx\<serial>".
	winUSBDeviceIDRE = regexp.MustCompile(`^USB\\VID_(\w+)&PID_(\w+)\\(.*)`)
	// Some vendors encode the real serial as "S/N:<hex>...".
	winUSBSerialRE = regexp.MustCompile(`^S/N:([0-9A-Fa-f]+)`)
	winUSBZerosRE  = regexp.MustCompile(`^0+$`)
)

// buildWinUSB maps CIM_LogicalDevice objects to USBDEVICES, mirroring
// Win32/USB.pm + Tools/USB.pm: the DeviceID yields VENDORID/PRODUCTID/SERIAL,
// invalid (zero) vendor ids are skipped, devices are deduplicated by
// vendor+product+serial, a pseudo serial containing "&" is dropped, and the
// usb.ids database fills MANUFACTURER (vendor) and NAME/CAPTION (device), falling
// back to the WMI Caption/Name. The vendor sub-module enrichment (docks, etc.)
// is follow-on.
func buildWinUSB(objects []map[string]any, db *USBIDs) []map[string]any {
	var devices []map[string]any
	seen := map[string]bool{}

	for _, o := range objects {
		m := winUSBDeviceIDRE.FindStringSubmatch(cimString(o, "DeviceID"))
		if m == nil {
			continue
		}
		vendorID, productID, serial := m[1], m[2], m[3]
		if s := winUSBSerialRE.FindStringSubmatch(serial); s != nil {
			serial = s[1]
		}

		// Skip invalid vendor ids.
		if vendorID == "" || winUSBZerosRE.MatchString(vendorID) {
			continue
		}

		dedup := vendorID + "-" + productID + "-" + serial
		if seen[dedup] {
			continue
		}
		seen[dedup] = true

		// A Windows-generated pseudo serial (contains "&") is not a real serial.
		if strings.Contains(serial, "&") {
			serial = ""
		}

		caption := cimString(o, "Caption")
		name := cimString(o, "Name")
		manufacturer := ""
		if vendor := db.Vendor(vendorID); vendor != "" {
			manufacturer = vendor
			if dev := db.Device(vendorID, productID); dev != "" {
				caption = dev
				name = dev
			}
		}

		device := map[string]any{"VENDORID": vendorID, "PRODUCTID": productID}
		setIf(device, "SERIAL", serial)
		setIf(device, "NAME", name)
		setIf(device, "CAPTION", caption)
		setIf(device, "MANUFACTURER", manufacturer)
		devices = append(devices, device)
	}
	return devices
}
