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

// dockerSeparator is the field separator Docker.pm embeds in its
// `docker ps -a --format` template.
const dockerSeparator = "#=#=#"

// DockerPSTemplate is the `docker ps -a --format` template (ID/Image/Ports/Names),
// matching Virtualization/Docker.pm.
const DockerPSTemplate = "{{.ID}}" + dockerSeparator + "{{.Image}}" + dockerSeparator + "{{.Ports}}" + dockerSeparator + "{{.Names}}"

// ParseDockerPS builds the base VIRTUALMACHINES entries from `docker ps -a`
// output (VMTYPE/UUID/IMAGE/NAME), mirroring Docker.pm::_getContainers. STATUS
// is filled per container by the collector (docker inspect).
func ParseDockerPS(out string) []map[string]any {
	var containers []map[string]any
	for _, line := range strings.Split(out, "\n") {
		if line == "" {
			continue
		}
		f := strings.Split(line, dockerSeparator)
		if len(f) != 4 {
			continue
		}
		containers = append(containers, map[string]any{
			"VMTYPE": "docker",
			"UUID":   f[0],
			"IMAGE":  f[1],
			"NAME":   f[3],
		})
	}
	return containers
}

var vboxListRE = regexp.MustCompile(`^"[^"]+"\s+\{([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\}$`)

// ParseVBoxList extracts the VM uuids from `VBoxManage list vms` output,
// mirroring _parseVBoxManageVms.
func ParseVBoxList(out string) []string {
	var uuids []string
	for _, line := range strings.Split(out, "\n") {
		if m := vboxListRE.FindStringSubmatch(strings.TrimSpace(line)); m != nil {
			uuids = append(uuids, m[1])
		}
	}
	return uuids
}

// vboxStatus maps a VBoxManage state to the GLPI virtual-machine STATUS values
// (Virtualization/VirtualBox.pm %status_list).
var vboxStatus = map[string]string{
	"powered off": "off", "saved": "off", "teleported": "off",
	"aborted": "crashed", "stuck": "blocked", "teleporting": "paused",
	"live snapshotting": "running", "starting": "running", "stopping": "dying",
	"saving": "dying", "restoring": "running", "running": "running", "paused": "paused",
}

var (
	vboxNameRE  = regexp.MustCompile(`^Name:\s+(.*)$`)
	vboxUUIDRE  = regexp.MustCompile(`^UUID:\s+(.+)`)
	vboxMemRE   = regexp.MustCompile(`^Memory size:\s+(.+)`)
	vboxStateRE = regexp.MustCompile(`^State:\s+(.+) \(`)
	vboxIndexRE = regexp.MustCompile(`^Index:\s+(\d+)$`)
)

// ParseVBoxShowVMInfo parses concatenated `VBoxManage showvminfo` output into
// VIRTUALMACHINES entries, mirroring _parseVBoxManage including the Index trick
// that disambiguates a VM Name from a USB-device Name.
func ParseVBoxShowVMInfo(out string) []map[string]any {
	var machines []map[string]any
	var cur map[string]any
	haveIndex := false

	for _, line := range strings.Split(out, "\n") {
		switch {
		case vboxNameRE.MatchString(line):
			if haveIndex {
				haveIndex = false // this Name belongs to a USB device, skip it
				continue
			}
			if cur != nil {
				machines = append(machines, cur)
			}
			cur = map[string]any{
				"NAME":      vboxNameRE.FindStringSubmatch(line)[1],
				"VCPU":      1,
				"SUBSYSTEM": "Oracle VM VirtualBox",
				"VMTYPE":    "virtualbox",
			}
		case cur == nil:
			continue
		case vboxUUIDRE.MatchString(line):
			cur["UUID"] = vboxUUIDRE.FindStringSubmatch(line)[1]
		case vboxMemRE.MatchString(line):
			if mb := canonicalSizeMB(strings.TrimSpace(vboxMemRE.FindStringSubmatch(line)[1])); mb > 0 {
				cur["MEMORY"] = mb
			}
		case vboxStateRE.MatchString(line):
			if st, ok := vboxStatus[vboxStateRE.FindStringSubmatch(line)[1]]; ok {
				cur["STATUS"] = st
			}
		case vboxIndexRE.MatchString(line):
			haveIndex = true
		}
	}
	if cur != nil {
		machines = append(machines, cur)
	}
	return machines
}

var xenLineRE = regexp.MustCompile(`^(.*\S)\s+(\d+)\s+(\d+)\s+(\d+)\s+([a-z-]{5,6})\s`)

// xenStatus maps a Xen state letter to a GLPI status (Xen.pm %status_list).
var xenStatus = map[string]string{
	"r": "running", "b": "blocked", "p": "paused",
	"s": "shutdown", "c": "crashed", "d": "dying",
}

// ParseXenList builds VIRTUALMACHINES entries from `xm list`/`xl list` output,
// mirroring Virtualization/Xen.pm: NAME/MEMORY/VCPU/STATUS/VMTYPE=xen,
// SUBSYSTEM=toolstack. Domain-0 and domain id 0 are skipped.
func ParseXenList(out, toolstack string) []map[string]any {
	var machines []map[string]any
	for _, line := range strings.Split(out, "\n") {
		m := xenLineRE.FindStringSubmatch(line)
		if m == nil {
			continue // header or non-matching line
		}
		name, vmid, memory, vcpu, state := m[1], m[2], m[3], m[4], m[5]
		if vmid == "0" || name == "Domain-0" {
			continue
		}
		status := "off"
		if stripped := strings.ReplaceAll(state, "-", ""); stripped != "" {
			if s, ok := xenStatus[stripped]; ok {
				status = s
			}
		}
		machines = append(machines, map[string]any{
			"NAME":      name,
			"MEMORY":    atoiOr0(memory),
			"VCPU":      atoiOr0(vcpu),
			"STATUS":    status,
			"VMTYPE":    "xen",
			"SUBSYSTEM": toolstack,
		})
	}
	return machines
}

// virtuozzoStatus maps a vzlist status to a GLPI status (Virtuozzo.pm).
var virtuozzoStatus = map[string]string{
	"stopped": "off", "running": "running", "paused": "paused",
	"mounted": "off", "suspended": "paused", "unknown": "off",
}

// ParseVirtuozzo builds VIRTUALMACHINES entries from `vzlist --all --no-header
// -o hostname,ctid,cpulimit,status,ostemplate`, mirroring Virtualization/
// Virtuozzo.pm (NAME/VCPU/STATUS/SUBSYSTEM/VMTYPE; MEMORY/MAC come from the
// container config and are pending).
func ParseVirtuozzo(out string) []map[string]any {
	var containers []map[string]any
	eachFields(out, func(f []string) {
		if len(f) < 5 {
			return
		}
		status := virtuozzoStatus[f[3]]
		if status == "" {
			status = "off"
		}
		containers = append(containers, map[string]any{
			"NAME":      f[0],
			"VCPU":      atoiOr0(f[2]),
			"STATUS":    status,
			"SUBSYSTEM": f[4],
			"VMTYPE":    "virtuozzo",
		})
	})
	return containers
}

func atoiOr0(s string) int {
	n, _ := strconv.Atoi(strings.TrimSpace(s))
	return n
}

// ParseMachinectl builds VIRTUALMACHINES entries from `machinectl --no-pager
// --no-legend`, mirroring Virtualization/SystemdNspawn.pm: name/class/service,
// skipping libvirt-qemu machines (covered by the libvirt collector).
func ParseMachinectl(out string) []map[string]any {
	var machines []map[string]any
	eachFields(out, func(f []string) {
		if len(f) < 3 {
			return
		}
		name, class, service := f[0], f[1], f[2]
		if service == "libvirt-qemu" {
			return
		}
		machines = append(machines, map[string]any{
			"NAME":      name,
			"VMTYPE":    service,
			"SUBSYSTEM": class,
			"VCPU":      0,
			"STATUS":    "running", // machinectl lists running machines
		})
	})
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
