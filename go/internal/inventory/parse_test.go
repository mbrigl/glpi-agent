// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestParseOSRelease(t *testing.T) {
	const osRelease = `NAME="Debian GNU/Linux"
VERSION_ID="13"
VERSION="13 (trixie)"
PRETTY_NAME="Debian GNU/Linux 13 (trixie)"
ID=debian
`
	os := ParseOSRelease(strings.NewReader(osRelease))
	if os["NAME"] != "Debian GNU/Linux" {
		t.Errorf("NAME = %v", os["NAME"])
	}
	if os["VERSION"] != "13 (trixie)" {
		t.Errorf("VERSION = %v", os["VERSION"])
	}
	if os["FULL_NAME"] != "Debian GNU/Linux 13 (trixie)" {
		t.Errorf("FULL_NAME = %v", os["FULL_NAME"])
	}
}

func TestParseMemInfo(t *testing.T) {
	const meminfo = `MemTotal:       32013004 kB
MemFree:         1000000 kB
SwapTotal:       8388604 kB
`
	mem, swap := ParseMemInfo(strings.NewReader(meminfo))
	if mem != 32013004/1024 {
		t.Errorf("memory = %d MiB", mem)
	}
	if swap != 8388604/1024 {
		t.Errorf("swap = %d MiB", swap)
	}
}

// TestParseCPUInfoGrouping checks the physical-id grouping: two logical
// processors on one package collapse to a single CPU with CORE/THREAD computed.
func TestParseCPUInfoGrouping(t *testing.T) {
	const cpuinfo = `processor	: 0
vendor_id	: GenuineIntel
cpu family	: 6
model		: 183
model name	: 13th Gen Intel(R) Core(TM) i7-13700
stepping	: 1
cpu MHz		: 2100.000
physical id	: 0
siblings	: 4
cpu cores	: 2

processor	: 1
vendor_id	: GenuineIntel
model name	: 13th Gen Intel(R) Core(TM) i7-13700
physical id	: 0
siblings	: 4
cpu cores	: 2

`
	cpus := ParseCPUInfo(strings.NewReader(cpuinfo))
	if len(cpus) != 1 {
		t.Fatalf("got %d CPUs, want 1 (grouped by physical id)", len(cpus))
	}
	c := cpus[0]
	if c["MANUFACTURER"] != "Intel" {
		t.Errorf("MANUFACTURER = %v, want Intel", c["MANUFACTURER"])
	}
	if c["CORE"] != 2 {
		t.Errorf("CORE = %v, want 2", c["CORE"])
	}
	if c["THREAD"] != 2 { // siblings 4 / cores 2
		t.Errorf("THREAD = %v, want 2", c["THREAD"])
	}
	if c["NAME"] != "13th Gen Intel(R) Core(TM) i7-13700" {
		t.Errorf("NAME = %v", c["NAME"])
	}
	if c["SPEED"] != 2100 {
		t.Errorf("SPEED = %v, want 2100", c["SPEED"])
	}
}

// TestParseCPUInfoNoPhysicalID covers a single CPU without a physical id.
func TestParseCPUInfoNoPhysicalID(t *testing.T) {
	const cpuinfo = `processor	: 0
vendor_id	: AuthenticAMD
model name	: AMD EPYC
`
	cpus := ParseCPUInfo(strings.NewReader(cpuinfo))
	if len(cpus) != 1 {
		t.Fatalf("got %d CPUs, want 1", len(cpus))
	}
	if cpus[0]["MANUFACTURER"] != "AMD" || cpus[0]["CORE"] != 1 || cpus[0]["THREAD"] != 1 {
		t.Errorf("cpu = %v", cpus[0])
	}
}
