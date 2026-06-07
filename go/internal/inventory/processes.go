// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"strconv"
	"strings"
	"time"
)

// clockTick is the kernel USER_HZ used to convert /proc starttime ticks to
// seconds (100 on Linux).
const clockTick = 100

// procStarttimeTicks extracts field 22 (starttime, in clock ticks) from a
// /proc/<pid>/stat line. The comm field (field 2) may contain spaces and
// parentheses, so fields are taken after the last ')'.
func procStarttimeTicks(stat string) int64 {
	i := strings.LastIndexByte(stat, ')')
	if i < 0 {
		return 0
	}
	fields := strings.Fields(stat[i+1:]) // fields[0] = state (field 3)
	const idx = 22 - 3                   // starttime is field 22
	if len(fields) <= idx {
		return 0
	}
	n, _ := strconv.ParseInt(fields[idx], 10, 64)
	return n
}

// computeStarted converts the boot epoch plus a process starttime (in clock
// ticks) to "YYYY-MM-DD HH:MM:SS".
func computeStarted(btime, startTicks int64) string {
	if btime == 0 || startTicks == 0 {
		return ""
	}
	return time.Unix(btime+startTicks/clockTick, 0).Format("2006-01-02 15:04:05")
}

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
