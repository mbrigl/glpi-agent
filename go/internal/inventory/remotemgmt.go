// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "regexp"

// teamViewerIDRE matches the client id in `teamviewer --info` output, mirroring
// the pattern in Remote_Mgmt/TeamViewer.pm (tolerating ANSI colour codes).
var teamViewerIDRE = regexp.MustCompile(`TeamViewer ID:(?:\x1b\[0m|\s)*(\d+)`)

// ParseTeamViewerInfo builds the TeamViewer REMOTE_MGMT entry from
// `teamviewer --info`, or nil when no id is found.
func ParseTeamViewerInfo(output string) map[string]any {
	m := teamViewerIDRE.FindStringSubmatch(output)
	if m == nil {
		return nil
	}
	return map[string]any{"ID": m[1], "TYPE": "teamviewer"}
}

var (
	anyDeskIDRE  = regexp.MustCompile(`(?m)^ad\.anynet\.id=(\S+)`)
	rustDeskIDRE = regexp.MustCompile(`(?m)^id\s*=\s*'(.*)'$`)
)

// ParseAnyDeskID builds the AnyDesk REMOTE_MGMT entry from an AnyDesk
// system.conf, mirroring Remote_Mgmt/AnyDesk.pm (ad.anynet.id).
func ParseAnyDeskID(conf string) map[string]any {
	m := anyDeskIDRE.FindStringSubmatch(conf)
	if m == nil {
		return nil
	}
	return map[string]any{"ID": m[1], "TYPE": "anydesk"}
}

// ParseRustDeskID builds the RustDesk REMOTE_MGMT entry from a RustDesk.toml,
// mirroring Remote_Mgmt/RustDesk.pm (id = '...').
func ParseRustDeskID(toml string) map[string]any {
	m := rustDeskIDRE.FindStringSubmatch(toml)
	if m == nil || m[1] == "" {
		return nil
	}
	return map[string]any{"ID": m[1], "TYPE": "rustdesk"}
}
