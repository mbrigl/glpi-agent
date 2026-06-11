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

	// videos (Win32_VideoController).
	if objs, err := powershellCIM("Win32_VideoController", winVideoProperties); err == nil {
		if v := buildWinVideos(objs); len(v) > 0 {
			s["VIDEOS"] = v
		}
	}

	// sounds (Win32_SoundDevice).
	if objs, err := powershellCIM("Win32_SoundDevice", winSoundProperties); err == nil {
		if v := buildWinSounds(objs); len(v) > 0 {
			s["SOUNDS"] = v
		}
	}

	// slots (Win32_SystemSlot).
	if objs, err := powershellCIM("Win32_SystemSlot", winSystemSlotProperties); err == nil {
		if v := buildWinSlots(objs); len(v) > 0 {
			s["SLOTS"] = v
		}
	}

	// ports (Win32_SerialPort + Win32_ParallelPort).
	var ports []map[string]any
	if objs, err := powershellCIM("Win32_SerialPort", winPortProperties); err == nil {
		ports = append(ports, buildWinPorts(objs, "Serial")...)
	}
	if objs, err := powershellCIM("Win32_ParallelPort", winPortProperties); err == nil {
		ports = append(ports, buildWinPorts(objs, "Parallel")...)
	}
	if len(ports) > 0 {
		s["PORTS"] = ports
	}

	// softwares (Uninstall registry keys, 64-bit + 32-bit views).
	if sw := collectWinSoftwares(); len(sw) > 0 {
		s["SOFTWARES"] = sw
	}

	// printers (Win32_Printer).
	if objs, err := powershellCIM("Win32_Printer", winPrinterProperties); err == nil {
		if p := buildWinPrinters(objs); len(p) > 0 {
			s["PRINTERS"] = p
		}
	}

	// processes (Win32_Process).
	if objs, err := powershellCIM("Win32_Process", winProcessProperties); err == nil {
		if p := buildWinProcesses(objs); len(p) > 0 {
			s["PROCESSES"] = p
		}
	}

	// antivirus (AntiVirusProduct in root/SecurityCenter + root/SecurityCenter2).
	var avObjs []map[string]any
	for _, ns := range []string{"root/SecurityCenter", "root/SecurityCenter2"} {
		if objs, err := powershellCIMNamespace(ns, "AntiVirusProduct", winAntivirusProperties); err == nil {
			avObjs = append(avObjs, objs...)
		}
	}
	if av := buildWinAntivirus(avObjs); len(av) > 0 {
		s["ANTIVIRUS"] = av
	}

	// environment (Win32_Environment system variables -> ENVS).
	if objs, err := powershellCIM("Win32_Environment", winEnvironmentProperties); err == nil {
		if e := buildWinEnvironment(objs); len(e) > 0 {
			s["ENVS"] = e
		}
	}

	// local users + groups (Win32_UserAccount / Win32_Group).
	if objs, err := powershellCIM("Win32_UserAccount", winLocalUserProperties); err == nil {
		if u := buildWinLocalUsers(objs); len(u) > 0 {
			s["LOCAL_USERS"] = u
		}
	}
	if objs, err := powershellCIM("Win32_Group", winLocalGroupProperties); err == nil {
		if g := buildWinLocalGroups(objs); len(g) > 0 {
			s["LOCAL_GROUPS"] = g
		}
	}

	// last logged-in user (Win32_ComputerSystem.UserName -> USERS + hardware).
	if lu := firstCIM("Win32_ComputerSystem", winLastUserProperties); lu != nil {
		if entry, login := buildWinLastUser(lu); entry != nil {
			s["USERS"] = []map[string]any{entry}
			s.mergeHardware(map[string]any{"LASTLOGGEDUSER": login})
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
// in the default root/CIMV2 namespace and returns the decoded CIM objects.
func powershellCIM(class string, props []string) ([]map[string]any, error) {
	return powershellCIMNamespace("", class, props)
}

// powershellCIMNamespace runs the CIM query in the given WMI namespace (empty
// for the default root/CIMV2), e.g. "root/SecurityCenter2" for AntiVirusProduct.
func powershellCIMNamespace(namespace, class string, props []string) ([]map[string]any, error) {
	ns := ""
	if namespace != "" {
		ns = fmt.Sprintf("-Namespace %s ", namespace)
	}
	script := fmt.Sprintf(
		"Get-CimInstance %s-ClassName %s | Select-Object %s | ConvertTo-Json -Compress -Depth 3",
		ns, class, strings.Join(props, ","),
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
