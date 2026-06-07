// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestBuildUsers(t *testing.T) {
	const passwd = `root:x:0:0:root:/root:/bin/bash
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
alice:x:1000:1000:Alice Example:/home/alice:/bin/zsh
+nisuser
`
	const group = `root:x:0:
sudo:x:27:alice
staff:x:1000:
`
	users, groups := BuildUsers(strings.NewReader(passwd), strings.NewReader(group))

	if len(users) != 3 {
		t.Fatalf("got %d users, want 3 (+nis line skipped)", len(users))
	}
	if users[2]["LOGIN"] != "alice" || users[2]["ID"] != "1000" ||
		users[2]["NAME"] != "Alice Example" || users[2]["HOME"] != "/home/alice" || users[2]["SHELL"] != "/bin/zsh" {
		t.Errorf("alice = %v", users[2])
	}

	// sudo has an explicit member; staff gets alice via primary gid 1000.
	byName := map[string]map[string]any{}
	for _, g := range groups {
		byName[g["NAME"].(string)] = g
	}
	if m := byName["sudo"]["MEMBER"].([]string); len(m) != 1 || m[0] != "alice" {
		t.Errorf("sudo members = %v, want [alice]", byName["sudo"]["MEMBER"])
	}
	if m, ok := byName["staff"]["MEMBER"].([]string); !ok || len(m) != 1 || m[0] != "alice" {
		t.Errorf("staff members = %v, want [alice] (primary group)", byName["staff"]["MEMBER"])
	}
}

func TestBuildEnvs(t *testing.T) {
	envs := BuildEnvs([]string{"PATH=/usr/bin", "HOME=/root", "EMPTY=", "=bad"})
	if len(envs) != 3 {
		t.Fatalf("got %d envs, want 3 (the =bad entry dropped)", len(envs))
	}
	if envs[0]["KEY"] != "PATH" || envs[0]["VAL"] != "/usr/bin" {
		t.Errorf("env[0] = %v", envs[0])
	}
	if envs[2]["KEY"] != "EMPTY" || envs[2]["VAL"] != "" {
		t.Errorf("empty value not preserved: %v", envs[2])
	}
}
