// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"strings"
)

// BuildUsers parses /etc/passwd and /etc/group into the LOCAL_USERS and
// LOCAL_GROUPS sections, mirroring Generic/Users.pm: users carry
// LOGIN/ID/NAME/HOME/SHELL, groups carry ID/NAME/MEMBER (explicit members plus
// users whose primary group is this group).
func BuildUsers(passwd, group io.Reader) (localUsers, localGroups []map[string]any) {
	// gid -> primary-group member logins
	primaryByGID := map[string][]string{}

	scanLines(passwd, func(line string) {
		if line == "" || line[0] == '#' || line[0] == '+' || line[0] == '-' {
			return
		}
		f := strings.Split(line, ":")
		if len(f) < 7 {
			return
		}
		login, uid, gid, gecos, home, shell := f[0], f[2], f[3], f[4], f[5], f[6]
		user := map[string]any{"LOGIN": login, "ID": uid}
		setIf(user, "NAME", gecos)
		setIf(user, "HOME", home)
		setIf(user, "SHELL", shell)
		localUsers = append(localUsers, user)
		primaryByGID[gid] = append(primaryByGID[gid], login)
	})

	scanLines(group, func(line string) {
		if line == "" || line[0] == '#' || line[0] == '+' || line[0] == '-' {
			return
		}
		f := strings.Split(line, ":")
		if len(f) < 4 {
			return
		}
		name, gid, members := f[0], f[2], f[3]
		g := map[string]any{"ID": gid, "NAME": name}

		var member []string
		if members != "" {
			member = append(member, strings.Split(members, ",")...)
		}
		member = append(member, primaryByGID[gid]...)
		if len(member) > 0 {
			g["MEMBER"] = member
		}
		localGroups = append(localGroups, g)
	})

	return localUsers, localGroups
}

func scanLines(r io.Reader, fn func(string)) {
	if r == nil {
		return
	}
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		fn(scanner.Text())
	}
}
