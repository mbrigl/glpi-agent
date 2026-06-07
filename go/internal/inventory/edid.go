// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	_ "embed"
	"encoding/base64"
	"fmt"
	"strconv"
	"strings"
	"sync"
)

// edidIDsData is the upstream EDID vendor database (share/edid.ids): lines of
// "CCC __ Vendor Name" mapping the 3-letter EISA id to a vendor.
//
//go:embed data/edid.ids
var edidIDsData string

var (
	edidVendorsOnce sync.Once
	edidVendors     map[string]string
)

func edidVendorDB() map[string]string {
	edidVendorsOnce.Do(func() {
		edidVendors = map[string]string{}
		for _, line := range strings.Split(edidIDsData, "\n") {
			code, name, ok := strings.Cut(line, " __ ")
			if ok && len(code) == 3 {
				edidVendors[code] = strings.TrimSpace(name)
			}
		}
	})
	return edidVendors
}

// EDID holds the fields decoded from a 128-byte EDID block.
type EDID struct {
	ManufacturerCode string // 3-letter EISA id
	SerialNumber     uint32 // mandatory numeric serial
	SerialText       string // optional ASCII serial (descriptor 0xFF)
	MonitorName      string // descriptor 0xFC
	Week             int
	Year             int
}

// ParseEDID decodes a raw EDID block, mirroring the fields Parse::EDID exposes
// that GLPI::Agent::Tools::Screen consumes. It returns false for a block without
// the standard header.
func ParseEDID(b []byte) (EDID, bool) {
	if len(b) < 128 {
		return EDID{}, false
	}
	header := []byte{0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00}
	for i, h := range header {
		if b[i] != h {
			return EDID{}, false
		}
	}

	var e EDID
	// Manufacturer: bytes 8-9, three 5-bit letters (A=1).
	m := uint16(b[8])<<8 | uint16(b[9])
	e.ManufacturerCode = string([]byte{
		byte((m>>10)&0x1f) + 'A' - 1,
		byte((m>>5)&0x1f) + 'A' - 1,
		byte(m&0x1f) + 'A' - 1,
	})
	// Serial: bytes 12-15, little-endian.
	e.SerialNumber = uint32(b[12]) | uint32(b[13])<<8 | uint32(b[14])<<16 | uint32(b[15])<<24
	e.Week = int(b[16])
	e.Year = 1990 + int(b[17])

	// Four 18-byte descriptors at 54/72/90/108.
	for _, off := range []int{54, 72, 90, 108} {
		if off+18 > len(b) {
			break
		}
		d := b[off : off+18]
		if d[0] != 0 || d[1] != 0 || d[2] != 0 {
			continue // detailed timing, not a monitor descriptor
		}
		text := descriptorText(d[5:18])
		switch d[3] {
		case 0xFC: // monitor name
			e.MonitorName = text
		case 0xFF: // monitor serial
			e.SerialText = text
		}
	}
	return e, true
}

// Manufacturer resolves the vendor name from the EISA code, falling back to the
// code itself (Tools/Screen::manufacturer via getEDIDVendor).
func (e EDID) Manufacturer() string {
	if v := edidVendorDB()[e.ManufacturerCode]; v != "" {
		return v
	}
	return e.ManufacturerCode
}

// Serial mirrors Tools/Screen::serial: the ASCII serial if present, otherwise
// the numeric serial as 8 hex digits.
func (e EDID) Serial() string {
	if e.SerialText != "" {
		return e.SerialText
	}
	return fmt.Sprintf("%08x", e.SerialNumber)
}

// WeekYear mirrors Tools/Screen::week_year_manufacture (week 255 is skipped).
func (e EDID) WeekYear() string {
	if e.Week == 255 {
		return strconv.Itoa(e.Year)
	}
	return fmt.Sprintf("%d/%d", e.Week, e.Year)
}

// BuildMonitor assembles one MONITORS entry from a raw EDID block, mirroring
// Generic/Screen.pm (_getEdidInfo + BASE64). Returns nil for an invalid block.
func BuildMonitor(raw []byte) map[string]any {
	e, ok := ParseEDID(raw)
	if !ok {
		return nil
	}
	monitor := map[string]any{
		"DESCRIPTION":  e.WeekYear(),
		"MANUFACTURER": e.Manufacturer(),
		"SERIAL":       e.Serial(),
		"BASE64":       base64.StdEncoding.EncodeToString(raw),
	}
	if caption := cleanCaption(e.MonitorName); caption != "" {
		monitor["CAPTION"] = caption
	}
	return monitor
}

// descriptorText decodes a 13-byte EDID descriptor string: terminated by 0x0A
// and right-padded with spaces.
func descriptorText(b []byte) string {
	s := string(b)
	if i := strings.IndexByte(s, 0x0A); i >= 0 {
		s = s[:i]
	}
	return strings.TrimRight(s, " ")
}

// cleanCaption mirrors Tools/Screen::caption cleanup (drop from the first
// non-printable character).
func cleanCaption(s string) string {
	for i := 0; i < len(s); i++ {
		if s[i] < 0x20 || s[i] > 0x7e {
			return strings.TrimSpace(s[:i])
		}
	}
	return strings.TrimSpace(s)
}
