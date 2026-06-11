// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"sort"
	"strings"
)

var (
	winPrinterProperties = []string{
		"ExtendedDetectedErrorState", "HorizontalResolution", "VerticalResolution",
		"Name", "Comment", "Description", "DriverName", "PortName", "Network",
		"Shared", "PrinterStatus", "ServerName", "ShareName", "PrintProcessor",
	}
	winProcessProperties = []string{"CommandLine", "ProcessId", "CreationDate", "CSName", "Name", "ExecutablePath"}

	// Win32_Printer.PrinterStatus -> STATUS (Win32/Printers.pm @status).
	winPrinterStatusVal = []string{
		"Unknown", "Other", "Unknown", "Idle", "Printing", "Warming Up",
		"Stopped printing", "Offline",
	}
)

// buildWinPrinters maps Win32_Printer to PRINTERS, mirroring Win32/Printers.pm:
// NAME/DRIVER/PORT, NETWORK/SHARED flags, STATUS from PrinterStatus, the
// COMMENT/DESCRIPTION/SERVERNAME/SHARENAME passthrough and RESOLUTION; entries
// are deduplicated by PortName and sorted by name. The USB serial and the
// ExtendedDetectedErrorState are follow-on.
func buildWinPrinters(objects []map[string]any) []map[string]any {
	seen := map[string]map[string]any{}
	for _, o := range objects {
		name := cimString(o, "Name")
		port := cimString(o, "PortName")
		if name == "" || port == "" {
			continue
		}
		if _, ok := seen[port]; ok && port != name {
			continue
		}

		p := map[string]any{
			"NAME":    name,
			"PORT":    port,
			"NETWORK": boolToInt(cimBool(o, "Network")),
			"SHARED":  boolToInt(cimBool(o, "Shared")),
		}
		setIf(p, "DRIVER", cimString(o, "DriverName"))
		setIf(p, "STATUS", enumAt(winPrinterStatusVal, cimInt(o, "PrinterStatus")))
		setIf(p, "PRINTPROCESSOR", cimString(o, "PrintProcessor"))
		for _, k := range []string{"Comment", "Description", "ServerName", "ShareName"} {
			setIf(p, strings.ToUpper(k), cimString(o, k))
		}
		if h := cimInt(o, "HorizontalResolution"); h > 0 {
			res := cimString(o, "HorizontalResolution")
			if cimInt(o, "VerticalResolution") > 0 {
				res += "x" + cimString(o, "VerticalResolution")
			}
			p["RESOLUTION"] = res
		}
		seen[port] = p
	}

	printers := make([]map[string]any, 0, len(seen))
	for _, p := range seen {
		printers = append(printers, p)
	}
	sort.Slice(printers, func(i, j int) bool {
		return strings.ToLower(printers[i]["NAME"].(string)) < strings.ToLower(printers[j]["NAME"].(string))
	})
	return printers
}

// buildWinProcesses maps Win32_Process to PROCESSES, mirroring Win32/Processes.pm:
// PID, CMD (CommandLine||Name), USER and STARTED (CreationDate ->
// "YYYY-MM-DD HH:MM:SS"). USER is built from the GetOwner User/Domain fields
// (merged in by the collector): it is "<login>@<domain>" except when the domain
// is empty, "NT AUTHORITY" or the local computer, in which case just "<login>";
// it falls back to the process Name when no owner is known. Processes with an
// empty CMD or USER are skipped. The CPU/MEM perf-counter fields are follow-on.
func buildWinProcesses(objects []map[string]any, computer string) []map[string]any {
	computer = strings.ToUpper(computer)
	var processes []map[string]any
	for _, o := range objects {
		pid := cimInt(o, "ProcessId")
		if pid == 0 {
			continue
		}
		cmd := winFirstNonEmpty(cimString(o, "CommandLine"), cimString(o, "Name"))
		user := winProcessUser(o, computer)
		if cmd == "" || user == "" {
			continue
		}
		proc := map[string]any{"PID": pid, "CMD": cmd, "USER": user}
		setIf(proc, "STARTED", wmiDateTime(cimString(o, "CreationDate")))
		processes = append(processes, proc)
	}
	return processes
}

// winProcessUser derives the PROCESSES USER field from the GetOwner User/Domain,
// appending "@<domain>" unless the domain is empty, "NT AUTHORITY" or the local
// computer, and falling back to the process Name when there is no owner.
func winProcessUser(o map[string]any, computer string) string {
	login := cimString(o, "User")
	domain := cimString(o, "Domain")
	user := login
	if domain != "" && domain != "NT AUTHORITY" && strings.ToUpper(domain) != computer {
		user += "@" + domain
	}
	if user == "" {
		user = cimString(o, "Name")
	}
	return user
}
