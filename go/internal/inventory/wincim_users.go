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

// winExplorerRE matches a process whose executable is Explorer.exe, identifying
// an interactive logged-in user (Win32/Users.pm _getLoggedUsers).
var winExplorerRE = regexp.MustCompile(`(?i)\\Explorer\.exe$`)

// buildWinLoggedUsers extracts the interactive logged-in users from the
// Win32_Process objects (those running Explorer.exe), mirroring Win32/Users.pm
// _getLoggedUsers: one {LOGIN, DOMAIN} per distinct owner (User/Domain from
// GetOwner, merged in by the collector), deduplicated by LOGIN.
func buildWinLoggedUsers(processes []map[string]any) []map[string]any {
	var users []map[string]any
	seen := map[string]bool{}
	for _, o := range processes {
		if !winExplorerRE.MatchString(cimString(o, "ExecutablePath")) {
			continue
		}
		login := cimString(o, "User")
		if login == "" || seen[login] {
			continue
		}
		seen[login] = true
		users = append(users, map[string]any{"LOGIN": login, "DOMAIN": cimString(o, "Domain")})
	}
	return users
}

// mergeWinUsers combines the last logged-in user with the currently logged-in
// users into the USERS section, mirroring Win32/Users.pm doInventory: the last
// user is listed first, then the logged users, deduplicated by
// lc(LOGIN)@lc(DOMAIN).
func mergeWinUsers(last map[string]any, logged []map[string]any) []map[string]any {
	var users []map[string]any
	seen := map[string]bool{}
	add := func(u map[string]any) {
		if u == nil {
			return
		}
		login, _ := u["LOGIN"].(string)
		domain, _ := u["DOMAIN"].(string)
		key := strings.ToLower(login) + "@" + strings.ToLower(domain)
		if seen[key] {
			return
		}
		seen[key] = true
		users = append(users, u)
	}
	add(last)
	for _, u := range logged {
		add(u)
	}
	return users
}
