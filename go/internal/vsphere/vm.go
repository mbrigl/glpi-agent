// SPDX-License-Identifier: GPL-2.0-only

package vsphere

import (
	"fmt"
	"regexp"
	"strings"

	"github.com/vmware/govmomi/vim25/mo"
	"github.com/vmware/govmomi/vim25/types"
)

// powerStateStatus maps a vSphere power state to the inventory STATUS values
// used by getVirtualMachines (STATUS_RUNNING/OFF/PAUSED).
var powerStateStatus = map[types.VirtualMachinePowerState]string{
	types.VirtualMachinePowerStatePoweredOn:  "running",
	types.VirtualMachinePowerStatePoweredOff: "off",
	types.VirtualMachinePowerStateSuspended:  "paused",
}

var uuidRE = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)

// virtualMachines mirrors getVirtualMachines: one entry per non-template VM with
// the same field set (NAME/STATUS/UUID/MEMORY/VMTYPE/VCPU/MAC/COMMENT, optional
// SERIAL and OPERATINGSYSTEM).
func virtualMachines(vms []mo.VirtualMachine) []map[string]any {
	var out []map[string]any
	for i := range vms {
		vm := &vms[i]
		if vm.Config == nil {
			continue
		}
		if vm.Config.Template {
			continue // templates are skipped, as in Perl
		}

		var macs []string
		for _, dev := range vm.Config.Hardware.Device {
			if eth, ok := dev.(types.BaseVirtualEthernetCard); ok {
				if mac := eth.GetVirtualEthernetCard().MacAddress; mac != "" {
					macs = append(macs, mac)
				}
			}
		}

		comment := vm.Config.Annotation
		// Preserve annotation line breaks as the Perl code does.
		comment = strings.ReplaceAll(comment, "\n", "&#10;")

		uuid := vm.Summary.Config.Uuid
		entry := map[string]any{
			"NAME":    vm.Name,
			"STATUS":  powerStateStatus[vm.Summary.Runtime.PowerState],
			"UUID":    uuid,
			"MEMORY":  vm.Summary.Config.MemorySizeMB,
			"VMTYPE":  "VMware",
			"VCPU":    vm.Summary.Config.NumCpu,
			"MAC":     strings.Join(macs, "/"),
			"COMMENT": comment,
		}
		if serial := vmwareSerial(uuid); serial != "" {
			entry["SERIAL"] = serial
		}
		if os := vmOperatingSystem(vm); os != nil {
			entry["OPERATINGSYSTEM"] = os
		}
		out = append(out, entry)
	}
	return out
}

// vmwareSerial reproduces the BIOS serial ESX computes from the VM uuid:
// "VMware-" + the 16 uuid bytes, space-separated, split 8/8 by a dash.
func vmwareSerial(uuid string) string {
	if !uuidRE.MatchString(uuid) {
		return ""
	}
	hexDigits := strings.ReplaceAll(uuid, "-", "")
	if len(hexDigits) != 32 {
		return ""
	}
	parts := make([]string, 16)
	for i := 0; i < 16; i++ {
		parts[i] = hexDigits[i*2 : i*2+2]
	}
	return fmt.Sprintf("VMware-%s-%s", strings.Join(parts[0:8], " "), strings.Join(parts[8:16], " "))
}

// vmOperatingSystem mirrors the OPERATINGSYSTEM block getVirtualMachines builds
// from guest information; it is kept only when a FULL_NAME is present.
func vmOperatingSystem(vm *mo.VirtualMachine) map[string]any {
	os := map[string]any{}
	if vm.Summary.Guest != nil {
		setIfNotEmpty(os, "FULL_NAME", vm.Summary.Guest.GuestFullName)
	}
	if vm.Guest != nil {
		setIfNotEmpty(os, "FQDN", vm.Guest.HostName)
		for _, n := range vm.Guest.Net {
			if n.DnsConfig != nil && n.DnsConfig.DomainName != "" {
				os["DNS_DOMAIN"] = n.DnsConfig.DomainName
				break
			}
		}
	}
	if bt := vm.Summary.Runtime.BootTime; bt != nil {
		os["BOOT_TIME"] = bt.Format("2006-01-02 15:04:05")
	}
	if full, ok := os["FULL_NAME"].(string); !ok || full == "" {
		return nil // Perl drops OPERATINGSYSTEM without a FULL_NAME
	}
	return os
}
