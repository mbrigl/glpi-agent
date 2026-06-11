// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import (
	"bytes"
	_ "embed"
	"sync"
)

// usbIDsData is the vendored usb.ids database (GPL-2.0), embedded only into the
// Windows build where the usb inventory needs it for vendor/device names.
//
//go:embed usb.ids
var usbIDsData []byte

var (
	usbIDsOnce sync.Once
	usbIDsDB   *USBIDs
)

func usbIDs() *USBIDs {
	usbIDsOnce.Do(func() {
		usbIDsDB = ParseUSBIDs(bytes.NewReader(usbIDsData))
	})
	return usbIDsDB
}

// collectWinUSB gathers the USBDEVICES section from CIM_LogicalDevice, resolving
// names against the embedded usb.ids database (Win32/USB.pm).
func collectWinUSB() []map[string]any {
	objs, err := powershellCIM("CIM_LogicalDevice", winUSBProperties)
	if err != nil {
		return nil
	}
	return buildWinUSB(objs, usbIDs())
}
