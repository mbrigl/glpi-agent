// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

// TestBuildWinLocalUsers checks the Win32_UserAccount -> LOCAL_USERS mapping:
// disabled, locked-out and non-local accounts are skipped.
func TestBuildWinLocalUsers(t *testing.T) {
	objs := []map[string]any{
		{"Name": "Administrator", "SID": "S-1-5-21-1-500", "Disabled": false, "Lockout": false, "LocalAccount": true},
		{"Name": "Disabled", "SID": "S-1-5-21-1-501", "Disabled": true, "Lockout": false, "LocalAccount": true},
		{"Name": "LockedOut", "SID": "S-1-5-21-1-502", "Disabled": false, "Lockout": true, "LocalAccount": true},
		{"Name": "DomainUser", "SID": "S-1-5-21-9-1000", "Disabled": false, "Lockout": false, "LocalAccount": false},
	}
	u := buildWinLocalUsers(objs)
	if len(u) != 1 {
		t.Fatalf("got %d local users, want 1", len(u))
	}
	if u[0]["NAME"] != "Administrator" || u[0]["ID"] != "S-1-5-21-1-500" {
		t.Errorf("user[0] = %v", u[0])
	}
}

// TestBuildWinLocalGroups checks the Win32_Group -> LOCAL_GROUPS mapping: the
// LocalAccount filter and the right-single-quote normalisation.
func TestBuildWinLocalGroups(t *testing.T) {
	objs := []map[string]any{
		{"Name": "Administrators", "SID": "S-1-5-32-544", "LocalAccount": true},
		{"Name": "Domain Admins", "SID": "S-1-5-21-9-512", "LocalAccount": false},
		{"Name": "L’étrange", "SID": "S-1-5-32-545", "LocalAccount": true},
	}
	g := buildWinLocalGroups(objs)
	if len(g) != 2 {
		t.Fatalf("got %d local groups, want 2", len(g))
	}
	if g[1]["NAME"] != "L'étrange" {
		t.Errorf("group[1] NAME = %q, want normalised quote", g[1]["NAME"])
	}
}

// TestBuildWinLastUser covers the UserName parsing forms.
func TestBuildWinLastUser(t *testing.T) {
	// DOMAIN\LOGIN form.
	entry, login := buildWinLastUser(map[string]any{"Name": "HOST", "UserName": `ACME\jdoe`})
	if entry["DOMAIN"] != "ACME" || entry["LOGIN"] != "jdoe" || login != "jdoe" {
		t.Errorf("domain form = %v / %q", entry, login)
	}

	// Dot domain is left as the raw UserName (upstream quirk).
	entry, _ = buildWinLastUser(map[string]any{"Name": "HOST", "UserName": `.\local`})
	if entry["DOMAIN"] != `.\local` || entry["LOGIN"] != "local" {
		t.Errorf("dot-domain form = %v", entry)
	}

	// No backslash: DOMAIN=UserName, LOGIN=computer name.
	entry, _ = buildWinLastUser(map[string]any{"Name": "HOST", "UserName": "plainuser"})
	if entry["DOMAIN"] != "plainuser" || entry["LOGIN"] != "HOST" {
		t.Errorf("plain form = %v", entry)
	}

	// No user set -> nil.
	if e, l := buildWinLastUser(map[string]any{"Name": "HOST"}); e != nil || l != "" {
		t.Errorf("empty = %v / %q", e, l)
	}
}
