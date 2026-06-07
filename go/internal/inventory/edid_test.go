// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// makeEDID builds a minimal valid 128-byte EDID block with manufacturer "SAM",
// the given numeric serial, week/year, and optional name/serial descriptors.
func makeEDID(serialNum uint32, week, yearOffset byte, name, serialText string) []byte {
	b := make([]byte, 128)
	copy(b, []byte{0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00})
	// "SAM" -> 0x4C2D.
	b[8], b[9] = 0x4C, 0x2D
	b[12] = byte(serialNum)
	b[13] = byte(serialNum >> 8)
	b[14] = byte(serialNum >> 16)
	b[15] = byte(serialNum >> 24)
	b[16] = week
	b[17] = yearOffset

	writeDescriptor := func(off int, tag byte, text string) {
		b[off+3] = tag
		data := []byte(text + "\n")
		copy(b[off+5:off+18], data)
	}
	if name != "" {
		writeDescriptor(54, 0xFC, name)
	}
	if serialText != "" {
		writeDescriptor(72, 0xFF, serialText)
	}
	return b
}

func TestParseEDID(t *testing.T) {
	raw := makeEDID(0x01020304, 10, 34, "SyncMaster", "ABC123")
	e, ok := ParseEDID(raw)
	if !ok {
		t.Fatal("valid EDID rejected")
	}
	if e.ManufacturerCode != "SAM" {
		t.Errorf("manufacturer code = %q, want SAM", e.ManufacturerCode)
	}
	if e.MonitorName != "SyncMaster" {
		t.Errorf("monitor name = %q", e.MonitorName)
	}
	if e.SerialText != "ABC123" {
		t.Errorf("serial text = %q", e.SerialText)
	}
	if e.WeekYear() != "10/2024" {
		t.Errorf("week/year = %q, want 10/2024", e.WeekYear())
	}
	// ASCII serial wins over the numeric one.
	if e.Serial() != "ABC123" {
		t.Errorf("serial = %q, want ABC123", e.Serial())
	}
}

func TestEDIDNumericSerialAndVendor(t *testing.T) {
	// No ASCII serial descriptor -> the numeric serial is used as 8 hex digits.
	raw := makeEDID(0x01020304, 255, 30, "", "")
	e, _ := ParseEDID(raw)
	if e.Serial() != "01020304" {
		t.Errorf("serial = %q, want 01020304", e.Serial())
	}
	// Week 255 is skipped: just the year.
	if e.WeekYear() != "2020" {
		t.Errorf("week/year = %q, want 2020", e.WeekYear())
	}
	// "SAM" resolves to a vendor name from the embedded edid.ids.
	if e.Manufacturer() == "" || e.Manufacturer() == "SAM" {
		t.Errorf("manufacturer = %q, want a resolved vendor name", e.Manufacturer())
	}
}

func TestBuildMonitor(t *testing.T) {
	raw := makeEDID(0x0a0b0c0d, 5, 33, "DELL U2415", "XYZ789")
	m := BuildMonitor(raw)
	if m == nil {
		t.Fatal("BuildMonitor returned nil for a valid block")
	}
	if m["CAPTION"] != "DELL U2415" || m["SERIAL"] != "XYZ789" || m["DESCRIPTION"] != "5/2023" {
		t.Errorf("monitor = %v", m)
	}
	if _, ok := m["BASE64"].(string); !ok || m["BASE64"] == "" {
		t.Errorf("BASE64 missing: %v", m["BASE64"])
	}
	if m["MANUFACTURER"] == "" {
		t.Errorf("MANUFACTURER empty")
	}

	// An invalid block (bad header) yields no monitor.
	if BuildMonitor(make([]byte, 128)) != nil {
		t.Error("expected nil for a block without the EDID header")
	}
}
