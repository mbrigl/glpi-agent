// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/xml"
	"regexp"
	"strconv"
	"strings"
)

var virshListRE = regexp.MustCompile(`^\s*(?:\d+|-)\s+(\S+)\s+(\S.*\S|\S)\s*$`)

// ParseVirshList parses `virsh --readonly list --all` into VIRTUALMACHINES base
// entries (NAME/STATUS/VMTYPE), mirroring Virtualization/Libvirt.pm::_parseList:
// the Xen Domain-0 is skipped and "shut off" becomes "off".
func ParseVirshList(out string) []map[string]any {
	var machines []map[string]any
	for _, line := range strings.Split(out, "\n") {
		if strings.HasPrefix(strings.TrimSpace(line), "Id") || strings.HasPrefix(strings.TrimSpace(line), "-----") {
			continue
		}
		m := virshListRE.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		name := m[1]
		if name == "Domain-0" {
			continue
		}
		status := strings.TrimPrefix(m[2], "shut off")
		if status == "" {
			status = "off"
		}
		machines = append(machines, map[string]any{
			"NAME":   name,
			"STATUS": status,
			"VMTYPE": "libvirt",
		})
	}
	return machines
}

type virshDomain struct {
	Type          string `xml:"type,attr"`
	UUID          string `xml:"uuid"`
	VCPU          string `xml:"vcpu"`
	CurrentMemory string `xml:"currentMemory"`
	Memory        string `xml:"memory"`
}

var memoryTailRE = regexp.MustCompile(`(\d+)\d{3}$`)

// ApplyVirshDumpXML merges the fields from `virsh --readonly dumpxml <name>`
// into a machine entry, mirroring _parseDumpxml: SUBSYSTEM (domain type), UUID,
// VCPU, and MEMORY (currentMemory with the trailing 3 digits dropped, KiB->MiB).
func ApplyVirshDumpXML(machine map[string]any, dump string) {
	var d virshDomain
	if err := xml.Unmarshal([]byte(dump), &d); err != nil {
		return
	}
	if d.Type != "" {
		machine["SUBSYSTEM"] = d.Type
	}
	if d.UUID != "" {
		machine["UUID"] = d.UUID
	}
	if v := strings.TrimSpace(d.VCPU); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			machine["VCPU"] = n
		}
	}
	mem := d.CurrentMemory
	if mem == "" {
		mem = d.Memory
	}
	if m := memoryTailRE.FindStringSubmatch(strings.TrimSpace(mem)); m != nil {
		if n, err := strconv.Atoi(m[1]); err == nil {
			machine["MEMORY"] = n
		}
	}
}
