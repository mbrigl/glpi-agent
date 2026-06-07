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

Handle 0x0900, DMI type 9, 17 bytes
System Slot Information
	Designation: PCIe x16
	Type: x16 PCI Express
	Current Usage: In Use
	ID: 1

Handle 0x0800, DMI type 8, 9 bytes
Port Connector Information
	Internal Reference Designator: JFP1
	External Reference Designator: USB1
	External Connector Type: Access Bus (USB)
	Port Type: USB
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

func TestBuildSlotsAndPorts(t *testing.T) {
	byType := ParseDmidecode(strings.NewReader(dmidecodeSample))

	slots := BuildSlots(byType)
	if len(slots) != 1 {
		t.Fatalf("slots = %d, want 1", len(slots))
	}
	s := slots[0]
	if s["NAME"] != "PCIe x16" || s["DESCRIPTION"] != "x16 PCI Express" || s["DESIGNATION"] != "1" {
		t.Errorf("slot = %v", s)
	}
	if s["STATUS"] != "used" { // "In Use" -> used
		t.Errorf("slot STATUS = %v, want used", s["STATUS"])
	}

	ports := BuildPorts(byType)
	if len(ports) != 1 {
		t.Fatalf("ports = %d, want 1", len(ports))
	}
	p := ports[0]
	// NAME prefers the internal reference designator; CAPTION the external one.
	if p["NAME"] != "JFP1" || p["CAPTION"] != "USB1" || p["TYPE"] != "USB" {
		t.Errorf("port = %v", p)
	}
	if p["DESCRIPTION"] != "Access Bus (USB)" { // first non-empty in the chain
		t.Errorf("port DESCRIPTION = %v", p["DESCRIPTION"])
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
