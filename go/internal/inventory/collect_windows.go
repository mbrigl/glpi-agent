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

	if objs, err := powershellCIM("Win32_OperatingSystem", winOSProperties); err == nil && len(objs) > 0 {
		s["OPERATINGSYSTEM"] = buildWinOS(objs[0])
	}

	return s
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
