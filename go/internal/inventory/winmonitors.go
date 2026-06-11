// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/base64"
	"regexp"
	"strings"
)

// winDesktopMonitorProperties are the Win32_DesktopMonitor properties for the
// monitor inventory (Generic/Screen.pm _getScreensFromWindows).
var winDesktopMonitorProperties = []string{
	"Caption", "MonitorManufacturer", "MonitorType", "PNPDeviceID", "Availability",
}

// buildWinMonitors maps Win32_DesktopMonitor objects (plus any extra screen ids
// from WMIMonitorConnectionParams) to MONITORS, mirroring Generic/Screen.pm
// _getScreensFromWindows + _getScreens: only available monitors (Availability==3)
// are kept, "Surface Display" panels are filtered out (along with their
// connection-param screens), the registry EDID block overrides
// CAPTION/DESCRIPTION/MANUFACTURER/SERIAL and adds BASE64, and screens are merged
// by SERIAL||BASE64. edid maps a screen id to its raw EDID bytes. PORT and
// ALTSERIAL are follow-on.
func buildWinMonitors(desktopMonitors []map[string]any, extraIDs []string, edid map[string][]byte) []map[string]any {
	type screen struct {
		id string
		m  map[string]any
	}
	var screens []*screen

	// Connection-param ids contribute EDID-only screens (e.g. a second monitor).
	for _, id := range extraIDs {
		screens = append(screens, &screen{id: id, m: map[string]any{}})
	}

	for _, o := range desktopMonitors {
		if cimInt(o, "Availability") != 3 {
			continue
		}
		pnp := cimString(o, "PNPDeviceID")
		if pnp == "" {
			continue
		}
		if strings.EqualFold(cimString(o, "MonitorType"), "Surface Display") {
			re := regexp.MustCompile(`(?i)^` + regexp.QuoteMeta(pnp) + `(_\d+)?$`)
			kept := make([]*screen, 0, len(screens))
			for _, s := range screens {
				if !re.MatchString(s.id) {
					kept = append(kept, s)
				}
			}
			screens = kept
			continue
		}
		m := map[string]any{}
		setIf(m, "NAME", cimString(o, "Caption"))
		setIf(m, "MANUFACTURER", cimString(o, "MonitorManufacturer"))
		setIf(m, "CAPTION", cimString(o, "Caption"))
		screens = append(screens, &screen{id: pnp, m: m})
	}

	var order []string
	byKey := map[string]map[string]any{}
	edidFields := []string{"CAPTION", "DESCRIPTION", "MANUFACTURER", "SERIAL"}

	for _, s := range screens {
		m := s.m
		if raw, ok := edid[s.id]; ok && len(raw) > 0 {
			if info := BuildMonitor(raw); info != nil {
				// A valid EDID block overrides these fields.
				for _, k := range edidFields {
					if v, ok := info[k]; ok {
						m[k] = v
					} else {
						delete(m, k)
					}
				}
				m["BASE64"] = info["BASE64"]
			} else {
				// Unparseable EDID: keep the WMI fields but still expose BASE64.
				m["BASE64"] = base64.StdEncoding.EncodeToString(raw)
			}
		}

		// Keep only screens carrying an EDID or a SERIAL+CAPTION pair.
		_, hasBase64 := m["BASE64"]
		if !hasBase64 && (m["SERIAL"] == nil || m["CAPTION"] == nil) {
			continue
		}

		key, _ := m["SERIAL"].(string)
		if key == "" {
			key, _ = m["BASE64"].(string)
		}
		if existing, ok := byKey[key]; ok {
			for k, v := range m {
				if _, set := existing[k]; !set {
					existing[k] = v
				}
			}
			continue
		}
		byKey[key] = m
		order = append(order, key)
	}

	var out []map[string]any
	for _, k := range order {
		out = append(out, byKey[k])
	}
	return out
}
