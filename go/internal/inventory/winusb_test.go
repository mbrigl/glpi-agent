// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"testing"
)

// loadTestUSBIDs parses the vendored usb.ids file (the same DB embedded into the
// Windows build) so the mapper can be pinned against real vendor/device names.
func loadTestUSBIDs(t *testing.T) *USBIDs {
	t.Helper()
	f, err := os.Open("usb.ids")
	if err != nil {
		t.Fatalf("open usb.ids: %v", err)
	}
	defer f.Close()
	return ParseUSBIDs(f)
}

// TestBuildWinUSB pins the CIM_LogicalDevice -> USBDEVICES mapping against the
// real usb.ids DB, using DeviceIDs and expected values taken from the upstream
// t/tasks/inventory/windows/usb.t cases (7, xppro2, bar-code-scanner). It
// exercises the db device-name override, the WMI-caption fallback when a device
// is absent from the DB, the S/N: serial extraction, the pseudo-serial ("&")
// drop, and the zero-vendor skip.
func TestBuildWinUSB(t *testing.T) {
	db := loadTestUSBIDs(t)

	objs := []map[string]any{
		// Hub with a pseudo serial -> serial dropped, name from DB.
		{"DeviceID": `USB\VID_8087&PID_0024\5&1234&0`, "Caption": "USB-Hub", "Name": "USB-Hub"},
		// QuickCam with S/N: serial.
		{"DeviceID": `USB\VID_046D&PID_08C9\S/N:6BE882AB`, "Caption": "wmi", "Name": "wmi"},
		// Bar code scanner: S/N:<hex> stops at the non-hex "_".
		{"DeviceID": `USB\VID_05E0&PID_1200\S/N:28A1CC69D1D8AE4585EDA53F7CD6CB88_REV:NBRMSAAHDM:01OCT15`, "Caption": "Bar Code Scanner", "Name": "Bar Code Scanner"},
		// CHERRY vendor present but product 0009 absent from DB -> WMI caption kept.
		{"DeviceID": `USB\VID_046A&PID_0009\6&abc&0`, "Caption": "Concentrador USB genérico", "Name": "Concentrador USB genérico"},
		// Invalid (zero) vendor -> skipped.
		{"DeviceID": `USB\VID_0000&PID_0000\x`, "Caption": "root hub", "Name": "root hub"},
		// Non-USB device id -> skipped.
		{"DeviceID": `PCI\VEN_8086&DEV_1234`, "Caption": "pci", "Name": "pci"},
	}

	got := buildWinUSB(objs, db)
	if len(got) != 4 {
		t.Fatalf("got %d usb devices, want 4", len(got))
	}

	want := []map[string]any{
		{"VENDORID": "8087", "PRODUCTID": "0024", "NAME": "Integrated Rate Matching Hub", "CAPTION": "Integrated Rate Matching Hub", "MANUFACTURER": "Intel Corp."},
		{"VENDORID": "046D", "PRODUCTID": "08C9", "SERIAL": "6BE882AB", "NAME": "QuickCam Ultra Vision", "CAPTION": "QuickCam Ultra Vision", "MANUFACTURER": "Logitech, Inc."},
		{"VENDORID": "05E0", "PRODUCTID": "1200", "SERIAL": "28A1CC69D1D8AE4585EDA53F7CD6CB88", "NAME": "Bar Code Scanner", "CAPTION": "Bar Code Scanner", "MANUFACTURER": "Symbol Technologies"},
		{"VENDORID": "046A", "PRODUCTID": "0009", "NAME": "Concentrador USB genérico", "CAPTION": "Concentrador USB genérico", "MANUFACTURER": "CHERRY"},
	}
	for i, w := range want {
		// No SERIAL expected unless present in want.
		if _, ok := w["SERIAL"]; !ok {
			if _, has := got[i]["SERIAL"]; has {
				t.Errorf("device[%d] unexpected SERIAL %v", i, got[i]["SERIAL"])
			}
		}
		for k, v := range w {
			if got[i][k] != v {
				t.Errorf("device[%d][%s] = %v, want %v", i, k, got[i][k], v)
			}
		}
	}
}

// TestBuildWinUSBDedup checks two devices sharing vendor+product+serial collapse
// to one entry.
func TestBuildWinUSBDedup(t *testing.T) {
	objs := []map[string]any{
		{"DeviceID": `USB\VID_046D&PID_C52B\AAAA`, "Caption": "x", "Name": "x"},
		{"DeviceID": `USB\VID_046D&PID_C52B\AAAA`, "Caption": "x", "Name": "x"},
	}
	if got := buildWinUSB(objs, nil); len(got) != 1 {
		t.Fatalf("dedup failed: got %d, want 1", len(got))
	}
}

// TestParseUSBIDs spot-checks the DB parser against known entries.
func TestParseUSBIDs(t *testing.T) {
	db := loadTestUSBIDs(t)
	if got := db.Vendor("8087"); got != "Intel Corp." {
		t.Errorf("vendor 8087 = %q", got)
	}
	if got := db.Device("8087", "0024"); got != "Integrated Rate Matching Hub" {
		t.Errorf("device 8087:0024 = %q", got)
	}
	// Case-insensitive lookup.
	if got := db.Device("05E0", "1200"); got != "Bar Code Scanner" {
		t.Errorf("device 05E0:1200 = %q", got)
	}
	// Absent device.
	if got := db.Device("046a", "0009"); got != "" {
		t.Errorf("device 046a:0009 = %q, want empty", got)
	}
}
