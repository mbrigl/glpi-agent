// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strconv"
	"strings"
)

var (
	winProcessorProperties = []string{
		"NumberOfCores", "NumberOfLogicalProcessors", "ProcessorId", "MaxClockSpeed",
		"SerialNumber", "Name", "Description", "Manufacturer",
	}
	winMemoryProperties = []string{
		"Capacity", "Caption", "Description", "FormFactor", "Removable", "Speed",
		"MemoryType", "SerialNumber",
	}

	// SMBIOS enum tables (Win32/Memory.pm @formFactorVal / @memoryTypeVal).
	winFormFactorVal = []string{
		"Unknown", "Other", "SIP", "DIP", "ZIP", "SOJ", "Proprietary", "SIMM",
		"DIMM", "TSOP", "PGA", "RIMM", "SODIMM", "SRIMM", "SMD", "SSMP", "QFP",
		"TQFP", "SOIC", "LCC", "PLCC", "BGA", "FPBGA", "LGA",
	}
	winMemoryTypeVal = []string{
		"Unknown", "Other", "DRAM", "Synchronous DRAM", "Cache DRAM", "EDO",
		"EDRAM", "VRAM", "SRAM", "RAM", "ROM", "Flash", "EEPROM", "FEPROM",
		"EPROM", "CDRAM", "3DRAM", "SDRAM", "SGRAM", "RDRAM", "DDR", "DDR-2",
	}
)

// buildWinCPUs maps Win32_Processor objects to CPUS entries, mirroring the WMI
// path of Win32/CPU.pm (the dmidecode and registry refinements — FAMILYNUMBER/
// MODEL/STEPPING from the registry Identifier — are follow-on).
func buildWinCPUs(procs []map[string]any) []map[string]any {
	var cpus []map[string]any
	for _, p := range procs {
		cpu := map[string]any{}
		setIf(cpu, "NAME", strings.TrimSpace(cimString(p, "Name")))
		setIf(cpu, "DESCRIPTION", cimString(p, "Description"))
		setIf(cpu, "MANUFACTURER", canonicalCPUManufacturer(cimString(p, "Manufacturer")))
		if serial := strings.ReplaceAll(cimString(p, "SerialNumber"), " ", ""); serial != "" {
			cpu["SERIAL"] = serial
		}
		setIf(cpu, "ID", cimString(p, "ProcessorId"))
		cores := cimInt(p, "NumberOfCores")
		if cores > 0 {
			cpu["CORE"] = cores
		}
		if speed := cimInt(p, "MaxClockSpeed"); speed > 0 {
			cpu["SPEED"] = speed
		}
		if logical := cimInt(p, "NumberOfLogicalProcessors"); cores > 0 && logical > 0 {
			cpu["THREAD"] = logical / cores
		}
		cpus = append(cpus, cpu)
	}
	return cpus
}

// buildWinMemories maps Win32_PhysicalMemory objects to MEMORIES entries,
// mirroring Win32/Memory.pm (the Win32_PhysicalMemoryArray MEMORYCORRECTION
// refinement is follow-on).
func buildWinMemories(mems []map[string]any) []map[string]any {
	var memories []map[string]any
	for i, m := range mems {
		mem := map[string]any{"NUMSLOTS": i + 1}
		if capMB := cimBytesToMB(m, "Capacity"); capMB > 0 {
			mem["CAPACITY"] = capMB
		}
		setIf(mem, "CAPTION", cimString(m, "Caption"))
		setIf(mem, "DESCRIPTION", cimString(m, "Description"))
		setIf(mem, "FORMFACTOR", enumAt(winFormFactorVal, cimInt(m, "FormFactor")))
		mem["REMOVABLE"] = boolToInt(cimBool(m, "Removable"))
		if sp := cimInt(m, "Speed"); sp > 0 {
			mem["SPEED"] = sp
		}
		setIf(mem, "TYPE", enumAt(winMemoryTypeVal, cimInt(m, "MemoryType")))
		setIf(mem, "SERIALNUMBER", cimString(m, "SerialNumber"))
		memories = append(memories, mem)
	}
	return memories
}

// canonicalCPUManufacturer resolves a CPU vendor string, mirroring the CPU map
// of Tools::getCanonicalManufacturer.
func canonicalCPUManufacturer(s string) string {
	switch s {
	case "GenuineIntel":
		return "Intel"
	case "AuthenticAMD":
		return "AMD"
	case "TMx86", "TransmetaCPU":
		return "Transmeta"
	case "CyrixInstead":
		return "Cyrix"
	case "CentaurHauls":
		return "VIA"
	case "HygonGenuine":
		return "Hygon"
	}
	return s
}

// cimInt reads an integer CIM property (a JSON number or numeric string).
func cimInt(obj map[string]any, key string) int {
	n, err := strconv.Atoi(cimString(obj, key))
	if err != nil {
		return 0
	}
	return n
}

// cimBool reads a boolean CIM property.
func cimBool(obj map[string]any, key string) bool {
	if b, ok := obj[key].(bool); ok {
		return b
	}
	return false
}

// enumAt returns the table entry at idx, or "" when out of range.
func enumAt(table []string, idx int) string {
	if idx < 0 || idx >= len(table) {
		return ""
	}
	return table[idx]
}
