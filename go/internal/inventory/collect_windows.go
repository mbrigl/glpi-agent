// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package inventory

import (
	"fmt"
	"os"
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

	// processes (Win32_Process with GetOwner). Queried once and reused for the
	// interactive logged-in USERS below.
	computer := cimString(cs, "Name")
	procObjs, _ := powershellProcessOwners()
	if p := buildWinProcesses(procObjs, computer); len(p) > 0 {
		s["PROCESSES"] = p
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

	// users: the last logged-in user (Win32_ComputerSystem.UserName) plus the
	// interactive logged-in users (Explorer.exe owners), merged + deduped.
	var lastEntry map[string]any
	if lu := firstCIM("Win32_ComputerSystem", winLastUserProperties); lu != nil {
		if entry, login := buildWinLastUser(lu); entry != nil {
			lastEntry = entry
			s.mergeHardware(map[string]any{"LASTLOGGEDUSER": login})
		}
	}
	if users := mergeWinUsers(lastEntry, buildWinLoggedUsers(procObjs)); len(users) > 0 {
		s["USERS"] = users
	}

	// inputs (Win32_Keyboard + Win32_PointingDevice).
	kbd, _ := powershellCIM("Win32_Keyboard", winKeyboardProperties)
	ptr, _ := powershellCIM("Win32_PointingDevice", winPointingProperties)
	if in := buildWinInputs(kbd, ptr); len(in) > 0 {
		s["INPUTS"] = in
	}

	// modems (Win32_POTSModem).
	if objs, err := powershellCIM("Win32_POTSModem", winModemProperties); err == nil {
		if m := buildWinModems(objs); len(m) > 0 {
			s["MODEMS"] = m
		}
	}

	// chassis type (Win32_SystemEnclosure.ChassisTypes -> hardware CHASSIS_TYPE).
	if ct := winChassis(firstCIM("Win32_SystemEnclosure", winEnclosureChassis)); ct != "" {
		s.mergeHardware(map[string]any{"CHASSIS_TYPE": ct})
	}

	// batteries (powercfg /batteryreport /xml -> BATTERIES).
	if b := collectWinBatteries(); len(b) > 0 {
		s["BATTERIES"] = b
	}

	// monitors (Win32_DesktopMonitor + root/wmi + registry EDID -> MONITORS).
	if m := collectWinMonitors(); len(m) > 0 {
		s["MONITORS"] = m
	}

	// firewall (per-profile EnableFirewall registry DWORD -> FIREWALL).
	if fw := collectWinFirewall(); len(fw) > 0 {
		s["FIREWALL"] = fw
	}

	// usb devices (CIM_LogicalDevice + embedded usb.ids -> USBDEVICES).
	if u := collectWinUSB(); len(u) > 0 {
		s["USBDEVICES"] = u
	}

	// licenses (Office registry product keys + SoftwareLicensingProduct WMI).
	if l := collectWinLicenses(); len(l) > 0 {
		s["LICENSEINFOS"] = l
	}

	return s
}

// collectWinBatteries runs `powercfg /batteryreport /xml` into a temp file and
// parses it, mirroring Win32/Batteries.pm _getBatteriesFromPowercfg.
func collectWinBatteries() []map[string]any {
	f, err := os.CreateTemp("", "batteries-*.xml")
	if err != nil {
		return nil
	}
	path := f.Name()
	f.Close()
	defer os.Remove(path)

	// We only care about the side effect (the generated XML file).
	_ = exec.Command("powercfg.exe", "/batteryreport", "/xml", "/output", path).Run()

	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	return parsePowercfgBatteries(data)
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

// powershellProcessOwners returns the Win32_Process objects enriched with the
// User/Domain of each process owner (the WMI GetOwner method, invoked via
// Invoke-CimMethod), mirroring the GetOwner lookup in Win32/Processes.pm and
// Win32/Users.pm.
func powershellProcessOwners() ([]map[string]any, error) {
	script := `Get-CimInstance Win32_Process | ForEach-Object {
  $o = $null
  try { $o = Invoke-CimMethod -InputObject $_ -MethodName GetOwner -ErrorAction Stop } catch {}
  [PSCustomObject]@{
    ProcessId      = $_.ProcessId
    Name           = $_.Name
    CommandLine    = $_.CommandLine
    CreationDate   = if ($_.CreationDate) { $_.CreationDate.ToString('yyyy-MM-dd HH:mm:ss') } else { $null }
    ExecutablePath = $_.ExecutablePath
    User           = if ($o) { $o.User } else { $null }
    Domain         = if ($o) { $o.Domain } else { $null }
  }
} | ConvertTo-Json -Compress -Depth 3`
	cmd := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script)
	out, err := cmd.Output()
	if err != nil {
		return nil, err
	}
	return decodeCIMJSON(out)
}

// winCIMDateNormalizer is a PowerShell pipeline stage that rewrites every
// [datetime] property of the piped objects to the canonical
// "yyyy-MM-dd HH:mm:ss" (local wall-clock) string before ConvertTo-Json.
// Without it, ConvertTo-Json serialises a [DateTime] in an ambiguous form
// (ISO-8601 or "/Date(ms)/" depending on the PowerShell edition); formatting at
// the source keeps the JSON deterministic and matches getFormatedWMIDateTime.
const winCIMDateNormalizer = `ForEach-Object { foreach ($p in $_.PSObject.Properties) { if ($p.Value -is [datetime]) { $p.Value = $p.Value.ToString('yyyy-MM-dd HH:mm:ss') } }; $_ }`

// powershellCIMNamespace runs the CIM query in the given WMI namespace (empty
// for the default root/CIMV2), e.g. "root/SecurityCenter2" for AntiVirusProduct.
func powershellCIMNamespace(namespace, class string, props []string) ([]map[string]any, error) {
	ns := ""
	if namespace != "" {
		ns = fmt.Sprintf("-Namespace %s ", namespace)
	}
	script := fmt.Sprintf(
		"Get-CimInstance %s-ClassName %s | Select-Object %s | %s | ConvertTo-Json -Compress -Depth 3",
		ns, class, strings.Join(props, ","), winCIMDateNormalizer,
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
