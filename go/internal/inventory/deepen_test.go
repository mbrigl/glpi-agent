// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"testing"
	"time"
)

func TestProcStarttimeAndStarted(t *testing.T) {
	// comm "(a b)c)" contains spaces and parens; state R is field 3, so
	// starttime (field 22) is index 19 of the fields after the last ')'.
	stat := "1234 (a b)c) R 1 1 1 0 -1 0 0 0 0 0 0 0 0 20 0 1 0 0 5000 12345 0"
	if got := procStarttimeTicks(stat); got != 5000 {
		t.Fatalf("starttime ticks = %d, want 5000", got)
	}

	// btime + ticks/100 seconds.
	const btime = int64(1_700_000_000)
	want := time.Unix(btime+5000/100, 0).Format("2006-01-02 15:04:05")
	if got := computeStarted(btime, 5000); got != want {
		t.Errorf("computeStarted = %q, want %q", got, want)
	}
	if computeStarted(0, 5000) != "" {
		t.Error("computeStarted with no btime should be empty")
	}
}

func TestParseMachinectl(t *testing.T) {
	const out = "alpine    container systemd-nspawn\n" +
		"winvm     vm        libvirt-qemu\n"
	m := ParseMachinectl(out)
	if len(m) != 1 {
		t.Fatalf("machines = %d, want 1 (libvirt-qemu skipped)", len(m))
	}
	if m[0]["NAME"] != "alpine" || m[0]["VMTYPE"] != "systemd-nspawn" || m[0]["SUBSYSTEM"] != "container" {
		t.Errorf("machine = %v", m[0])
	}
	if m[0]["STATUS"] != "running" {
		t.Errorf("STATUS = %v, want running", m[0]["STATUS"])
	}
}

func TestParseXenList(t *testing.T) {
	const out = `Name                                        ID   Mem VCPUs      State   Time(s)
Domain-0                                     0  4096     4     r-----   1234.5
web                                          1  2048     2     -b----    567.8
db                                           2  1024     1     --p---     12.3
`
	vms := ParseXenList(out, "xl")
	if len(vms) != 2 {
		t.Fatalf("vms = %d, want 2 (header + Domain-0 skipped)", len(vms))
	}
	if vms[0]["NAME"] != "web" || vms[0]["MEMORY"] != 2048 || vms[0]["VCPU"] != 2 {
		t.Errorf("web = %v", vms[0])
	}
	if vms[0]["STATUS"] != "blocked" || vms[0]["VMTYPE"] != "xen" || vms[0]["SUBSYSTEM"] != "xl" {
		t.Errorf("web status/type = %v", vms[0])
	}
	if vms[1]["STATUS"] != "paused" {
		t.Errorf("db STATUS = %v, want paused", vms[1]["STATUS"])
	}
}

func TestParseVirtuozzo(t *testing.T) {
	const out = "web    101  2  running  centos-7-x86_64\n" +
		"db     102  4  stopped  debian-12-x86_64\n"
	c := ParseVirtuozzo(out)
	if len(c) != 2 {
		t.Fatalf("containers = %d, want 2", len(c))
	}
	if c[0]["NAME"] != "web" || c[0]["VCPU"] != 2 || c[0]["STATUS"] != "running" {
		t.Errorf("web = %v", c[0])
	}
	if c[0]["SUBSYSTEM"] != "centos-7-x86_64" || c[0]["VMTYPE"] != "virtuozzo" {
		t.Errorf("web subsys = %v", c[0])
	}
	if c[1]["STATUS"] != "off" { // stopped -> off
		t.Errorf("db STATUS = %v, want off", c[1]["STATUS"])
	}
}

func TestParseCrowdStrikeVersion(t *testing.T) {
	av := ParseCrowdStrikeVersion("version = 7.10.16208.0\n")
	if av == nil || av["NAME"] != "CrowdStrike Falcon Sensor" || av["VERSION"] != "7.10.16208.0" {
		t.Errorf("av = %v", av)
	}
	if av["ENABLED"] != 1 || av["COMPANY"] != "CrowdStrike" {
		t.Errorf("av flags = %v", av)
	}
	if ParseCrowdStrikeVersion("no version here") != nil {
		t.Error("expected nil without a version")
	}
}

func TestParseAnyDeskAndRustDesk(t *testing.T) {
	ad := ParseAnyDeskID("ad.security.enabled=true\nad.anynet.id=123456789\n")
	if ad == nil || ad["ID"] != "123456789" || ad["TYPE"] != "anydesk" {
		t.Errorf("anydesk = %v", ad)
	}

	rd := ParseRustDeskID("rendezvous_server = ''\nid = '987654321'\n")
	if rd == nil || rd["ID"] != "987654321" || rd["TYPE"] != "rustdesk" {
		t.Errorf("rustdesk = %v", rd)
	}
	// Empty id (RustDesk >=1.2 moves it out of the config) -> nil.
	if ParseRustDeskID("id = ''\n") != nil {
		t.Error("empty RustDesk id should yield nil")
	}
}
