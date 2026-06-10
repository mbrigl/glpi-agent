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
