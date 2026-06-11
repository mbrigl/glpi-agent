// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strings"
)

var (
	// winLocalUserProperties / winLocalGroupProperties cover the WMI lookups in
	// Tools/Win32/Users.pm getUsers() and Win32/Users.pm _getLocalGroups(); the
	// Disabled/Lockout/LocalAccount filtering is applied in the pure mapper.
	winLocalUserProperties  = []string{"Domain", "Name", "SID", "Disabled", "Lockout", "LocalAccount"}
	winLocalGroupProperties = []string{"Name", "SID", "LocalAccount"}
	// winLastUserProperties feed Win32/Users.pm _getLastUser (Win32_ComputerSystem).
	winLastUserProperties = []string{"Name", "UserName"}

	winUserNameRE = regexp.MustCompile(`^([^\\]*)\\(.*)$`)
)

// buildWinLocalUsers maps Win32_UserAccount to LOCAL_USERS, mirroring
// Tools/Win32/Users.pm getUsers(localusers => 1): only enabled, non-locked local
// accounts are kept, as NAME/ID(SID) pairs.
func buildWinLocalUsers(objects []map[string]any) []map[string]any {
	var out []map[string]any
	for _, o := range objects {
		if cimBool(o, "Disabled") || cimBool(o, "Lockout") || !cimBool(o, "LocalAccount") {
			continue
		}
		name := cimString(o, "Name")
		if name == "" {
			continue
		}
		out = append(out, map[string]any{"NAME": name, "ID": cimString(o, "SID")})
	}
	return out
}

// buildWinLocalGroups maps Win32_Group (LocalAccount) to LOCAL_GROUPS, mirroring
// Win32/Users.pm _getLocalGroups: NAME/ID(SID), with the right-single-quotation
// mark normalised to a plain quote.
func buildWinLocalGroups(objects []map[string]any) []map[string]any {
	var out []map[string]any
	for _, o := range objects {
		if !cimBool(o, "LocalAccount") {
			continue
		}
		name := strings.ReplaceAll(cimString(o, "Name"), "’", "'")
		if name == "" {
			continue
		}
		out = append(out, map[string]any{"NAME": name, "ID": cimString(o, "SID")})
	}
	return out
}

// buildWinLastUser maps Win32_ComputerSystem to the last logged-in USERS entry,
// mirroring Win32/Users.pm _getLastUser: UserName "DOMAIN\LOGIN" is split into
// DOMAIN/LOGIN (a "." domain is left as the raw UserName, as upstream does), and
// LOGIN is returned for the obsolete hardware LASTLOGGEDUSER field. The AzureAD
// and registry fallbacks are follow-on. Returns nil/"" when no user is set.
func buildWinLastUser(cs map[string]any) (entry map[string]any, lastLogin string) {
	name := cimString(cs, "Name")
	userName := cimString(cs, "UserName")
	if name == "" || userName == "" {
		return nil, ""
	}
	domain, login := userName, name
	if m := winUserNameRE.FindStringSubmatch(userName); m != nil {
		if m[1] != "." {
			domain = m[1]
		}
		login = m[2]
	}
	return map[string]any{"DOMAIN": domain, "LOGIN": login}, login
}
