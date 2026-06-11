// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/base64"
	"testing"
)

// TestBuildWinMonitors checks the Win32_DesktopMonitor + registry-EDID mapping:
// the EDID overrides the WMI fields, an EDID-only second screen is added from the
// connection-param ids, unavailable monitors are skipped, and a Surface Display
// panel filters itself (and its connection-param screen) out.
func TestBuildWinMonitors(t *testing.T) {
	edidA := makeEDID(0x01020304, 10, 34, "SyncMaster", "EDIDSERIALA")
	edidB := makeEDID(0x0A0B0C0D, 5, 33, "DELL U2412", "EDIDSERIALB")

	desktop := []map[string]any{
		// Available primary monitor with an EDID override.
		{"Caption": "Generic PnP Monitor", "MonitorManufacturer": "(Standard)", "PNPDeviceID": `DISPLAY\SAM0001\A`, "Availability": float64(3)},
		// Unavailable -> skipped.
		{"Caption": "Off", "PNPDeviceID": `DISPLAY\OFF\X`, "Availability": float64(8)},
		// Surface Display -> filters out its own connection-param screen.
		{"Caption": "Surface", "MonitorType": "Surface Display", "PNPDeviceID": `DISPLAY\SURF\S`, "Availability": float64(3)},
	}
	extraIDs := []string{`DISPLAY\DELL0002\B`, `DISPLAY\SURF\S`}
	edid := map[string][]byte{
		`DISPLAY\SAM0001\A`:  edidA,
		`DISPLAY\DELL0002\B`: edidB,
	}

	mons := buildWinMonitors(desktop, extraIDs, edid)
	if len(mons) != 2 {
		t.Fatalf("got %d monitors, want 2", len(mons))
	}

	// Order: extra (EDID-only) screen first, then the desktop monitor.
	dellU := mons[0]
	if dellU["SERIAL"] != "EDIDSERIALB" || dellU["CAPTION"] != "DELL U2412" {
		t.Errorf("dell monitor = %v", dellU)
	}
	if dellU["BASE64"] != base64.StdEncoding.EncodeToString(edidB) {
		t.Errorf("dell BASE64 mismatch")
	}

	sam := mons[1]
	// EDID caption overrides the WMI "Generic PnP Monitor" caption.
	if sam["CAPTION"] != "SyncMaster" || sam["SERIAL"] != "EDIDSERIALA" {
		t.Errorf("samsung monitor = %v", sam)
	}
	// The Surface Display connection-param screen must have been filtered out.
	for _, m := range mons {
		if m["BASE64"] == nil && m["SERIAL"] == nil {
			t.Errorf("unexpected empty monitor: %v", m)
		}
	}
}

// TestBuildWinMonitorsNoEDID keeps a monitor that has a SERIAL+CAPTION even
// without an EDID block, and drops one with neither.
func TestBuildWinMonitorsNoEDID(t *testing.T) {
	desktop := []map[string]any{
		{"Caption": "NoEdid", "PNPDeviceID": `DISPLAY\X\1`, "Availability": float64(3)},
	}
	if mons := buildWinMonitors(desktop, nil, nil); len(mons) != 0 {
		t.Fatalf("monitor without EDID/SERIAL should be dropped, got %d", len(mons))
	}
}
