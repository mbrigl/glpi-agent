// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import (
	"fmt"
	"os/exec"
	"strings"
)

// Collect gathers the local Windows inventory via WMI/CIM (PowerShell
// Get-CimInstance), mirroring the upstream Task/Inventory/Win32/* modules. Only
// the operatingsystem category is wired so far; more categories follow.
func Collect() Sections {
	s := Sections{}
	s.mergeHardware(map[string]any{"NAME": hostname()})

	// Query each CIM class once and reuse the first object across categories.
	osObj := firstCIM("Win32_OperatingSystem", winOSProperties)
	cs := firstCIM("Win32_ComputerSystem", winCSProperties)

	if osObj != nil {
		s["OPERATINGSYSTEM"] = buildWinOS(osObj)
	}

	// hardware (Win32_OperatingSystem + Win32_ComputerSystem + product UUID).
	csProduct := firstCIM("Win32_ComputerSystemProduct", winCSProductProperties)
	s.mergeHardware(buildWinHardware(osObj, cs, csProduct))

	// bios (Win32_Bios + ComputerSystem + SystemEnclosure + BaseBoard).
	s["BIOS"] = buildWinBios(
		firstCIM("Win32_Bios", winBiosProperties),
		cs,
		firstCIM("Win32_SystemEnclosure", winEnclosureProperties),
		firstCIM("Win32_BaseBoard", winBaseBoardProperties),
	)

	// cpus (one entry per Win32_Processor).
	if procs, err := powershellCIM("Win32_Processor", winProcessorProperties); err == nil && len(procs) > 0 {
		s["CPUS"] = buildWinCPUs(procs)
	}

	// memories (one entry per Win32_PhysicalMemory).
	if mems, err := powershellCIM("Win32_PhysicalMemory", winMemoryProperties); err == nil && len(mems) > 0 {
		s["MEMORIES"] = buildWinMemories(mems)
	}

	// drives (Win32_LogicalDisk volumes).
	if disks, err := powershellCIM("Win32_LogicalDisk", winLogicalDiskProperties); err == nil && len(disks) > 0 {
		s["DRIVES"] = buildWinDrives(disks, cimString(osObj, "SystemDrive"))
	}

	// storages (Win32_DiskDrive physical disks).
	if disks, err := powershellCIM("Win32_DiskDrive", winDiskDriveProperties); err == nil && len(disks) > 0 {
		s["STORAGES"] = buildWinStorages(disks)
	}

	// controllers (PCI devices across the PnP/controller WMI classes).
	var controllerObjs []map[string]any
	for _, class := range winControllerClasses {
		if objs, err := powershellCIM(class, winControllerProperties); err == nil {
			controllerObjs = append(controllerObjs, objs...)
		}
	}
	if c := buildWinControllers(controllerObjs); len(c) > 0 {
		s["CONTROLLERS"] = c
	}

	// networks (Win32_NetworkAdapter joined with its configuration, per IP).
	adapters, aErr := powershellCIM("Win32_NetworkAdapter", winNetAdapterProperties)
	cfgs, cErr := powershellCIM("Win32_NetworkAdapterConfiguration", winNetConfigProperties)
	if aErr == nil && cErr == nil {
		if n := buildWinNetworks(adapters, cfgs); len(n) > 0 {
			s["NETWORKS"] = n
		}
	}

	return s
}

// firstCIM runs a CIM query and returns the first object, or nil on error/empty.
func firstCIM(class string, props []string) map[string]any {
	objs, err := powershellCIM(class, props)
	if err != nil || len(objs) == 0 {
		return nil
	}
	return objs[0]
}

// powershellCIM runs `Get-CimInstance <class> | Select <props> | ConvertTo-Json`
// and returns the decoded CIM objects.
func powershellCIM(class string, props []string) ([]map[string]any, error) {
	script := fmt.Sprintf(
		"Get-CimInstance -ClassName %s | Select-Object %s | ConvertTo-Json -Compress -Depth 3",
		class, strings.Join(props, ","),
	)
	cmd := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script)
	out, err := cmd.Output()
	if err != nil {
		return nil, err
	}
	objs, err := decodeCIMJSON(out)
	if err != nil {
		return nil, fmt.Errorf("decoding %s CIM JSON: %w", class, err)
	}
	return objs, nil
}
