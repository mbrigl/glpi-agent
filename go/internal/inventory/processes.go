// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"strconv"
	"strings"
)

// ProcStatus holds the fields read from /proc/<pid>/status.
type ProcStatus struct {
	Name     string
	Uid      string
	VmSizeKB int
	VmRSSKB  int
}

// ParseProcStatus parses a /proc/<pid>/status stream.
func ParseProcStatus(r io.Reader) ProcStatus {
	var st ProcStatus
	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		key, val, ok := strings.Cut(scanner.Text(), ":")
		if !ok {
			continue
		}
		val = strings.TrimSpace(val)
		switch key {
		case "Name":
			st.Name = val
		case "Uid":
			if f := strings.Fields(val); len(f) > 0 {
				st.Uid = f[0] // real uid
			}
		case "VmSize":
			st.VmSizeKB = firstIntField(val)
		case "VmRSS":
			st.VmRSSKB = firstIntField(val)
		}
	}
	return st
}

// processEntry builds one PROCESSES entry, mirroring the field set of
// Generic/Processes.pm (USER/PID/VIRTUALMEMORY/MEM/CMD). The starttime-derived
// STARTED and CPUUSAGE columns are follow-on.
func processEntry(pid string, st ProcStatus, cmdline string, uidToUser map[string]string) map[string]any {
	entry := map[string]any{"PID": pid}

	user := uidToUser[st.Uid]
	if user == "" {
		user = st.Uid
	}
	setIf(entry, "USER", user)

	cmd := strings.TrimSpace(strings.ReplaceAll(cmdline, "\x00", " "))
	if cmd == "" {
		cmd = st.Name // kernel threads have an empty cmdline
	}
	setIf(entry, "CMD", cmd)

	if st.VmSizeKB > 0 {
		entry["VIRTUALMEMORY"] = st.VmSizeKB
	}
	if st.VmRSSKB > 0 {
		entry["MEM"] = st.VmRSSKB
	}
	return entry
}

func firstIntField(s string) int {
	if f := strings.Fields(s); len(f) > 0 {
		n, _ := strconv.Atoi(f[0])
		return n
	}
	return 0
}
