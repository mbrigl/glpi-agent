// SPDX-License-Identifier: GPL-2.0-only

package inventory

// BuildLoggedUsers parses `who --users` output into the USERS section (the
// currently logged-in users), mirroring Generic/Users.pm::_getLoggedUsers: one
// LOGIN per session line (first field).
func BuildLoggedUsers(whoOutput string) []map[string]any {
	var users []map[string]any
	eachFields(whoOutput, func(f []string) {
		if login := f[0]; login != "" {
			users = append(users, map[string]any{"LOGIN": login})
		}
	})
	return users
}
