// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestParseProcStatusAndEntry(t *testing.T) {
	const status = `Name:	bash
Umask:	0022
State:	S (sleeping)
Tgid:	1234
Pid:	1234
Uid:	1000	1000	1000	1000
VmSize:	  12345 kB
VmRSS:	   6789 kB
`
	st := ParseProcStatus(strings.NewReader(status))
	if st.Name != "bash" || st.Uid != "1000" || st.VmSizeKB != 12345 || st.VmRSSKB != 6789 {
		t.Fatalf("status = %+v", st)
	}

	entry := processEntry("1234", st, "bash\x00-l\x00", map[string]string{"1000": "alice"})
	if entry["PID"] != "1234" || entry["USER"] != "alice" {
		t.Errorf("entry id/user wrong: %v", entry)
	}
	if entry["CMD"] != "bash -l" {
		t.Errorf("CMD = %v, want 'bash -l'", entry["CMD"])
	}
	if entry["VIRTUALMEMORY"] != 12345 || entry["MEM"] != 6789 {
		t.Errorf("entry mem wrong: %v", entry)
	}
}

func TestProcessEntryKernelThread(t *testing.T) {
	// A kernel thread has an empty cmdline -> CMD falls back to Name, and an
	// unknown uid -> USER is the numeric uid.
	st := ParseProcStatus(strings.NewReader("Name:\tkworker\nUid:\t0\t0\t0\t0\n"))
	entry := processEntry("2", st, "", map[string]string{})
	if entry["CMD"] != "kworker" {
		t.Errorf("CMD = %v, want kworker", entry["CMD"])
	}
	if entry["USER"] != "0" {
		t.Errorf("USER = %v, want 0", entry["USER"])
	}
}
