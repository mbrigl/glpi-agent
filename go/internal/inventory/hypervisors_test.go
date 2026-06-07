// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestParseQemuCmd(t *testing.T) {
	const cmd = "/usr/bin/qemu-system-x86_64 -name vm1,debug-threads=on -m 2048 -smp 4 " +
		"-uuid 12345678-1234-1234-1234-123456789abc -enable-kvm"
	vm := ParseQemuCmd(cmd)
	if vm["NAME"] != "vm1" || vm["MEMORY"] != 2048 || vm["VCPU"] != 4 {
		t.Errorf("qemu = %v", vm)
	}
	if vm["UUID"] != "12345678-1234-1234-1234-123456789abc" || vm["VMTYPE"] != "kvm" {
		t.Errorf("qemu uuid/type = %v", vm)
	}

	// -m with a unit and -smp with cores/threads/sockets.
	vm2 := ParseQemuCmd("qemu-system-arm -name a -m 1G -smp cores=2,threads=2,sockets=1")
	if vm2["MEMORY"] != 1024 || vm2["VCPU"] != 4 || vm2["VMTYPE"] != "qemu" {
		t.Errorf("qemu2 = %v", vm2)
	}
}

func TestParseLxdListInfoConfig(t *testing.T) {
	const list = `+------+---------+------+------+-----------+-----------+
| NAME |  STATE  | IPV4 | IPV6 |   TYPE    | SNAPSHOTS |
+------+---------+------+------+-----------+-----------+
| web  | RUNNING | ...  |      | CONTAINER | 0         |
+------+---------+------+------+-----------+-----------+
| db   | STOPPED | ...  |      | CONTAINER | 0         |
+------+---------+------+------+-----------+-----------+`
	names := ParseLxdList(list)
	if len(names) != 2 || names[0] != "web" || names[1] != "db" {
		t.Fatalf("names = %v", names)
	}

	if got := ParseLxdInfoStatus("Name: web\nStatus: RUNNING\nType: container\n"); got != "running" {
		t.Errorf("status = %q, want running", got)
	}

	vcpu, mem := ParseLxdConfig("config:\n  limits.cpu: \"4\"\n  limits.memory: 2GB\n")
	if vcpu != 4 || mem != 2048 {
		t.Errorf("config vcpu=%d mem=%d, want 4/2048", vcpu, mem)
	}
}

func TestParseLxcState(t *testing.T) {
	if got := ParseLxcState("Name: web\nState: RUNNING\nPID: 1234\n"); got != "running" {
		t.Errorf("state = %q, want running", got)
	}
	if got := ParseLxcState("State: FROZEN\n"); got != "paused" {
		t.Errorf("state = %q, want paused", got)
	}
	if got := ParseLxcState("State: STOPPED\n"); got != "off" {
		t.Errorf("state = %q, want off", got)
	}
}

func TestParseVserverStatus(t *testing.T) {
	if ParseVserverStatus("Vserver 'web' is running") != "running" {
		t.Error("running not detected")
	}
	if ParseVserverStatus("Vserver 'db' is stopped") != "off" {
		t.Error("stopped not detected")
	}
}
