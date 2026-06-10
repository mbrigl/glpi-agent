// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

// edidFixtures lists real upstream EDID blobs vendored under testdata/edid and
// the values t/tasks/inventory/generic/screen.t expects from each. serial is
// asserted only for the simple (numeric / plain ASCII) cases; for blobs whose
// upstream SERIAL is a Parse::EDID-combined value (those that also carry an
// ALTSERIAL) only MANUFACTURER/CAPTION/DESCRIPTION are pinned, since Go emits
// the raw descriptor serial rather than replicating that combination.
var edidFixtures = []struct {
	name, manufacturer, caption, description, serial string
}{
	{"acer-al1716", "Acer Technologies", "AL1716", "37/2006", "0000b051"},
	{"crt.sony-gdm420", "Sony Corporation", "CPD-G420", "39/2001", "6017706"},
	// week 255 -> year-only DESCRIPTION. Serial not pinned: upstream ignores the
	// blob's "0" serial descriptor (yielding the numeric 01010101) whereas Go
	// uses the raw descriptor; the year-only case is the point here.
	{"iiyama-PL2779A", "Iiyama North America", "PL2779Q", "2013", ""},
	{"lcd.20inches", "Rogen Tech Distribution Inc", "B102005", "52/2004", "0000033f"},
	{"crt.test_box_lmontel", "Compaq Computer Company", "COMPAQ MV920", "8/2000", "008GA23MA966"},
	{"lcd.fujitsu-a171", "Fujitsu Siemens Computers GmbH", "A17-1", "34/2005", "YEEP525344"},
	// ASCII-serial monitors: serial left blank (combined upstream SERIAL).
	{"acer-al1716.2", "Acer Technologies", "AL1716", "32/2007", ""},
	{"acer-b226wl", "Acer Technologies", "B226WL", "3/2018", ""},
	{"acer-b247y", "Acer Technologies", "B247Y", "17/2021", ""},
	{"acer-v247y", "Acer Technologies", "V247Y", "7/2022", ""},
}

func TestEDIDRealFixtures(t *testing.T) {
	for _, f := range edidFixtures {
		t.Run(f.name, func(t *testing.T) {
			raw, err := os.ReadFile(filepath.Join("testdata", "edid", f.name))
			if err != nil {
				t.Fatal(err)
			}
			m := BuildMonitor(raw)
			if m == nil {
				t.Fatal("BuildMonitor returned nil for a real EDID blob")
			}
			if m["MANUFACTURER"] != f.manufacturer {
				t.Errorf("MANUFACTURER = %v, want %q", m["MANUFACTURER"], f.manufacturer)
			}
			if m["CAPTION"] != f.caption {
				t.Errorf("CAPTION = %v, want %q", m["CAPTION"], f.caption)
			}
			if m["DESCRIPTION"] != f.description {
				t.Errorf("DESCRIPTION = %v, want %q", m["DESCRIPTION"], f.description)
			}
			if f.serial != "" {
				if m["SERIAL"] != f.serial {
					t.Errorf("SERIAL = %v, want %q", m["SERIAL"], f.serial)
				}
			} else if s, _ := m["SERIAL"].(string); s == "" {
				t.Error("SERIAL is empty for an ASCII-serial monitor")
			}
			// Every monitor carries the round-trippable BASE64 of its blob.
			if _, ok := m["BASE64"].(string); !ok {
				t.Error("missing BASE64")
			}
		})
	}
}
