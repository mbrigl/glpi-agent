// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestParseVirshListAndDumpXML(t *testing.T) {
	const list = ` Id   Name       State
----------------------------
 1    web        running
 -    db         shut off
 -    Domain-0   running
`
	vms := ParseVirshList(list)
	if len(vms) != 2 {
		t.Fatalf("vms = %d, want 2 (Domain-0 skipped)", len(vms))
	}
	if vms[0]["NAME"] != "web" || vms[0]["STATUS"] != "running" || vms[0]["VMTYPE"] != "libvirt" {
		t.Errorf("web = %v", vms[0])
	}
	if vms[1]["NAME"] != "db" || vms[1]["STATUS"] != "off" { // "shut off" -> "off"
		t.Errorf("db = %v", vms[1])
	}

	const dump = `<domain type='kvm'>
  <name>web</name>
  <uuid>4dea22b3-1d52-d8f3-2516-782e98ab3fa0</uuid>
  <memory unit='KiB'>4194304</memory>
  <currentMemory unit='KiB'>2097152</currentMemory>
  <vcpu placement='static'>2</vcpu>
</domain>`
	ApplyVirshDumpXML(vms[0], dump)
	if vms[0]["SUBSYSTEM"] != "kvm" || vms[0]["UUID"] != "4dea22b3-1d52-d8f3-2516-782e98ab3fa0" {
		t.Errorf("web after xml = %v", vms[0])
	}
	if vms[0]["VCPU"] != 2 {
		t.Errorf("VCPU = %v, want 2", vms[0]["VCPU"])
	}
	// currentMemory 2097152 KiB -> drop trailing 3 digits -> 2097 MiB.
	if vms[0]["MEMORY"] != 2097 {
		t.Errorf("MEMORY = %v, want 2097", vms[0]["MEMORY"])
	}
}

func TestParseRPMQA(t *testing.T) {
	// NAME ARCH VERSION-RELEASE INSTALLTIME SIZE VENDOR SUMMARY GROUP
	const out = "bash\tx86_64\t5.2.15-1.fc39\t1700000000\t8388608\tFedora Project\tThe GNU Bourne Again shell\tSystem/Shells\n" +
		"nano\tx86_64\t7.2-4.fc39\t1700100000\t2097152\t(none)\tA small text editor\tApplications/Editors\n"
	sw := ParseRPMQA(out)
	if len(sw) != 2 {
		t.Fatalf("rpm packages = %d, want 2", len(sw))
	}
	bash := sw[0]
	if bash["NAME"] != "bash" || bash["ARCH"] != "x86_64" || bash["VERSION"] != "5.2.15-1.fc39" {
		t.Errorf("bash = %v", bash)
	}
	if bash["FROM"] != "rpm" || bash["FILESIZE"] != 8388608 || bash["SYSTEM_CATEGORY"] != "System/Shells" {
		t.Errorf("bash detail = %v", bash)
	}
	if bash["PUBLISHER"] != "Fedora Project" || bash["COMMENTS"] != "The GNU Bourne Again shell" {
		t.Errorf("bash vendor/summary = %v", bash)
	}
	if bash["INSTALLDATE"] == nil || bash["INSTALLDATE"] == "" {
		t.Errorf("INSTALLDATE missing: %v", bash["INSTALLDATE"])
	}
	// VENDOR "(none)" -> no PUBLISHER.
	if _, present := sw[1]["PUBLISHER"]; present {
		t.Errorf("nano should have no PUBLISHER: %v", sw[1]["PUBLISHER"])
	}
}
