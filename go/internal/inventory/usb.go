// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"path/filepath"
)

// BuildUSB collects the USBDEVICES section from <root>/sys/bus/usb/devices,
// mirroring Generic/USB.pm: a device needs both a vendor and a product id, and a
// serial shorter than 5 chars is dropped. Fields: VENDORID, PRODUCTID, SERIAL,
// CLASS, SUBCLASS, NAME, CAPTION, MANUFACTURER. (Perl reads `lsusb -v`; the same
// values are exposed in sysfs.)
func BuildUSB(root string) []map[string]any {
	matches, _ := invFS.Glob(filepath.Join(root, "sys/bus/usb/devices/*"))

	var devices []map[string]any
	for _, dir := range matches {
		vendor := readSysLine(filepath.Join(dir, "idVendor"))
		product := readSysLine(filepath.Join(dir, "idProduct"))
		if vendor == "" || product == "" {
			continue // interface node or non-USB entry
		}
		device := map[string]any{"VENDORID": vendor, "PRODUCTID": product}
		setIf(device, "CLASS", readSysLine(filepath.Join(dir, "bDeviceClass")))
		setIf(device, "SUBCLASS", readSysLine(filepath.Join(dir, "bDeviceSubClass")))
		setIf(device, "MANUFACTURER", readSysLine(filepath.Join(dir, "manufacturer")))
		if name := readSysLine(filepath.Join(dir, "product")); name != "" {
			device["NAME"] = name
			device["CAPTION"] = name
		}
		if serial := readSysLine(filepath.Join(dir, "serial")); len(serial) >= 5 {
			device["SERIAL"] = serial
		}
		devices = append(devices, device)
	}
	return devices
}
