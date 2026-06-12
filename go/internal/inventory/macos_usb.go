// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strconv"
	"strings"
)

var ioregPropRE = regexp.MustCompile(`"([^"]+)" = (<?"[^"]+">?|<?[0-9a-fA-F]+>?|\{.*\})$`)

// parseIORegDevices parses `ioreg -c <class> -r -l -w0 -d1` output into per-device
// attribute maps, mirroring Tools/MacOS.pm getIODevices: a device block starts at
// a "<class <class>,>" line and ends at the first "| }"; scalar `"key" = value`
// properties are collected (angle/quote wrappers stripped; nested `{...}` hashes
// are kept raw as they are unused here).
func parseIORegDevices(text, class string) []map[string]any {
	classRE := regexp.MustCompile(`<class ` + regexp.QuoteMeta(class) + `,`)
	var devices []map[string]any
	var device map[string]any

	for _, line := range strings.Split(text, "\n") {
		if classRE.MatchString(line) {
			if device != nil {
				devices = append(devices, device)
			}
			device = map[string]any{}
			continue
		}
		if device == nil {
			continue
		}
		if strings.Contains(line, "| }") {
			devices = append(devices, device)
			device = nil
			continue
		}
		if m := ioregPropRE.FindStringSubmatch(line); m != nil {
			value := m[2]
			if s := strings.TrimSuffix(strings.TrimPrefix(value, "<"), ">"); s != value && !strings.HasPrefix(value, "<?") {
				value = s
			}
			value = strings.Trim(value, `"`)
			device[m[1]] = value
		}
	}
	if device != nil {
		devices = append(devices, device)
	}
	return devices
}

// macDec2Hex formats a decimal value as "0x<hex>", mirroring Tools.pm dec2hex
// (a value already prefixed with 0x is returned unchanged).
func macDec2Hex(value string) string {
	if value == "" {
		return ""
	}
	if strings.HasPrefix(value, "0x") {
		return value
	}
	n, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return value
	}
	return "0x" + strconv.FormatInt(n, 16)
}

// buildMacUSB maps IOUSBDevice ioreg blocks to USBDEVICES, mirroring
// MacOS/USB.pm _getDevices: VENDORID/PRODUCTID (dec2hex), SERIAL/NAME from the
// USB strings, CLASS/SUBCLASS. The doInventory SERIAL dedup is applied by the
// collector.
func buildMacUSB(devices []map[string]any) []map[string]any {
	var out []map[string]any
	for _, d := range devices {
		usb := map[string]any{
			"VENDORID":  macDec2Hex(plistStr(d, "idVendor")),
			"PRODUCTID": macDec2Hex(plistStr(d, "idProduct")),
		}
		setIf(usb, "SERIAL", plistStr(d, "USB Serial Number"))
		setIf(usb, "NAME", plistStr(d, "USB Product Name"))
		setIf(usb, "CLASS", plistStr(d, "bDeviceClass"))
		setIf(usb, "SUBCLASS", plistStr(d, "bDeviceSubClass"))
		out = append(out, usb)
	}
	return out
}
