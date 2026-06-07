// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

const dmidecodeSample = `# dmidecode 3.5
Getting SMBIOS data from sysfs.

Handle 0x1000, DMI type 16, 23 bytes
Physical Memory Array
	Location: System Board Or Motherboard
	Error Correction Type: None
	Maximum Capacity: 64 GB

Handle 0x1100, DMI type 17, 92 bytes
Memory Device
	Array Handle: 0x1000
	Size: 16 GB
	Form Factor: DIMM
	Locator: DIMM 0
	Type: DDR4
	Speed: 3200 MT/s
	Manufacturer: Samsung
	Serial Number: 0A1B2C3D

Handle 0x1101, DMI type 17, 92 bytes
Memory Device
	Size: No Module Installed
	Locator: DIMM 1
	Type: Unknown
`

func TestParseDmidecodeAndMemories(t *testing.T) {
	byType := ParseDmidecode(strings.NewReader(dmidecodeSample))
	if len(byType[17]) != 2 {
		t.Fatalf("type 17 records = %d, want 2", len(byType[17]))
	}

	mem := BuildMemories(byType)
	if len(mem) != 2 {
		t.Fatalf("memories = %d, want 2", len(mem))
	}

	m0 := mem[0]
	if m0["NUMSLOTS"] != 1 || m0["CAPTION"] != "DIMM 0" || m0["TYPE"] != "DDR4" {
		t.Errorf("dimm0 = %v", m0)
	}
	if m0["CAPACITY"] != 16*1024 {
		t.Errorf("CAPACITY = %v, want %d MiB", m0["CAPACITY"], 16*1024)
	}
	if m0["SPEED"] != 3200 {
		t.Errorf("SPEED = %v, want 3200", m0["SPEED"])
	}
	if m0["SERIALNUMBER"] != "0A1B2C3D" || m0["MANUFACTURER"] != "Samsung" {
		t.Errorf("dimm0 serial/mfr = %v", m0)
	}
	// MEMORYCORRECTION comes from the type 16 array.
	if m0["MEMORYCORRECTION"] != "None" {
		t.Errorf("MEMORYCORRECTION = %v, want None", m0["MEMORYCORRECTION"])
	}

	// Empty slot: NUMSLOTS set, no CAPACITY.
	if mem[1]["NUMSLOTS"] != 2 {
		t.Errorf("empty slot NUMSLOTS = %v", mem[1]["NUMSLOTS"])
	}
	if _, present := mem[1]["CAPACITY"]; present {
		t.Errorf("empty slot must not have CAPACITY: %v", mem[1])
	}
}

func TestCanonicalSizeAndSpeed(t *testing.T) {
	cases := map[string]int{"16 GB": 16384, "8192 MB": 8192, "2 TB": 2 * 1024 * 1024}
	for in, want := range cases {
		if got := canonicalSizeMB(in); got != want {
			t.Errorf("canonicalSizeMB(%q) = %d, want %d", in, got, want)
		}
	}
	if got := canonicalSpeed("3200 MT/s"); got != 3200 {
		t.Errorf("canonicalSpeed = %d, want 3200", got)
	}
}
