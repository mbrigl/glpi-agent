// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package collect

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// WMICollector implements "getFromWMI" (Collect/WMI.pm): query a WMI class for a
// set of properties via PowerShell Get-CimInstance.
type WMICollector struct{}

func (WMICollector) Function() string { return "getFromWMI" }

func (WMICollector) Validation() map[string]any {
	return map[string]any{"class": Mandatory, "properties": Mandatory}
}

// Results queries the WMI class and returns one record per object with the
// requested properties.
func (WMICollector) Results(job map[string]any, log *logging.Logger) []map[string]any {
	class := str(job["class"])
	props := wmiProperties(job["properties"])
	if class == "" || len(props) == 0 {
		return nil
	}

	script := fmt.Sprintf(
		"Get-CimInstance -ClassName %s | Select-Object %s | ConvertTo-Json -Compress -Depth 3",
		class, strings.Join(props, ","),
	)
	out, err := exec.Command("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script).Output()
	if err != nil {
		log.Debug("getFromWMI: " + err.Error())
		return nil
	}

	objects := decodeWMIObjects(out)
	var results []map[string]any
	for _, obj := range objects {
		result := map[string]any{}
		for _, p := range props {
			if v, ok := obj[p]; ok && v != nil {
				result[p] = fmt.Sprintf("%v", v)
			}
		}
		if len(result) > 0 {
			results = append(results, result)
		}
	}
	return results
}

// wmiProperties normalises the job's properties (an array, or a single comma/
// space-separated string) into a list.
func wmiProperties(v any) []string {
	switch t := v.(type) {
	case []any:
		var out []string
		for _, e := range t {
			out = append(out, splitProps(str(e))...)
		}
		return out
	case string:
		return splitProps(t)
	default:
		return nil
	}
}

func splitProps(s string) []string {
	var out []string
	for _, p := range strings.FieldsFunc(s, func(r rune) bool { return r == ',' || r == ' ' }) {
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

// decodeWMIObjects decodes ConvertTo-Json output (a bare object or an array).
func decodeWMIObjects(data []byte) []map[string]any {
	data = []byte(strings.TrimSpace(string(data)))
	if len(data) == 0 {
		return nil
	}
	if data[0] == '[' {
		var arr []map[string]any
		_ = json.Unmarshal(data, &arr)
		return arr
	}
	var obj map[string]any
	if json.Unmarshal(data, &obj) == nil {
		return []map[string]any{obj}
	}
	return nil
}
