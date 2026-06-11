// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"strings"
)

// USBIDs holds the vendor/device names parsed from a usb.ids database, mirroring
// the lookup side of Tools/Generic.pm getUSBDeviceVendor.
type USBIDs struct {
	vendors map[string]usbVendor
}

type usbVendor struct {
	name    string
	devices map[string]string
}

// ParseUSBIDs parses a usb.ids file (vendor lines "<id>  <name>", device lines
// "\t<id>  <name>"). Only the vendor and device levels are kept; interface and
// class lines are ignored. Ids are stored lowercased.
func ParseUSBIDs(r io.Reader) *USBIDs {
	db := &USBIDs{vendors: map[string]usbVendor{}}
	var vendorID string

	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 64*1024), 1<<20)
	for scanner.Scan() {
		line := scanner.Text()
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		switch {
		case strings.HasPrefix(line, "\t\t"):
			// Interface line - ignored.
		case strings.HasPrefix(line, "\t"):
			// Device line: "\t<id>  <name>".
			id, name, ok := splitUSBLine(line[1:])
			if ok && vendorID != "" {
				if v, exists := db.vendors[vendorID]; exists {
					v.devices[id] = name
				}
			}
		case line[0] == 'C':
			// Class line - ends the vendor context for device association.
			vendorID = ""
		default:
			// Vendor line: "<id>  <name>".
			id, name, ok := splitUSBLine(line)
			if ok {
				vendorID = id
				db.vendors[id] = usbVendor{name: name, devices: map[string]string{}}
			}
		}
	}
	return db
}

// splitUSBLine splits a "<id>  <name>" line into a lowercased 4-hex id and the
// trimmed name.
func splitUSBLine(line string) (id, name string, ok bool) {
	fields := strings.SplitN(line, "  ", 2)
	if len(fields) != 2 {
		// Fall back to first run of whitespace.
		if i := strings.IndexAny(line, " \t"); i > 0 {
			fields = []string{line[:i], strings.TrimLeft(line[i:], " \t")}
		} else {
			return "", "", false
		}
	}
	id = strings.ToLower(strings.TrimSpace(fields[0]))
	name = strings.TrimSpace(fields[1])
	if len(id) != 4 || name == "" {
		return "", "", false
	}
	return id, name, true
}

// Vendor returns the vendor name for a (lowercased) vendor id, or "".
func (db *USBIDs) Vendor(id string) string {
	if db == nil {
		return ""
	}
	return db.vendors[strings.ToLower(id)].name
}

// Device returns the device name for a (lowercased) vendor+product id, or "".
func (db *USBIDs) Device(vendorID, productID string) string {
	if db == nil {
		return ""
	}
	v, ok := db.vendors[strings.ToLower(vendorID)]
	if !ok {
		return ""
	}
	return v.devices[strings.ToLower(productID)]
}
