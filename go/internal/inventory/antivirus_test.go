// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestParseBitdefender(t *testing.T) {
	const out = `Product version: 7.8.4.200
Engines version: 7.95301
Antimalware status: On
New product update available: no
New security content available: yes
Last security content update: 2026-06-05 at 10:00`
	av := ParseBitdefender(out)
	if av["VERSION"] != "7.8.4.200" || av["BASE_VERSION"] != "7.95301" {
		t.Errorf("versions = %v", av)
	}
	if av["ENABLED"] != 1 {
		t.Errorf("ENABLED = %v, want 1", av["ENABLED"])
	}
	// "New security content available: yes" -> not up to date.
	if av["UPTODATE"] != 0 {
		t.Errorf("UPTODATE = %v, want 0", av["UPTODATE"])
	}
	if av["BASE_CREATION"] != "2026-06-05" {
		t.Errorf("BASE_CREATION = %v", av["BASE_CREATION"])
	}
}

func TestParseSentinelOne(t *testing.T) {
	const out = `Agent version: 23.4.2.14
DFI library version: 2024.1.0
Agent state: Enabled
Connectivity: On`
	av := ParseSentinelOne(out)
	if av["VERSION"] != "23.4.2.14" || av["BASE_VERSION"] != "2024.1.0" {
		t.Errorf("versions = %v", av)
	}
	if av["ENABLED"] != 1 || av["UPTODATE"] != 1 {
		t.Errorf("flags = %v", av)
	}
}

func TestParseDrWeb(t *testing.T) {
	av := ParseDrWeb("drweb-ctl 13.0.0.202312061\n", "active\n", "Last update: 2026-06-01\n")
	if av["VERSION"] != "13.0.0.202312061" || av["ENABLED"] != 1 || av["BASE_VERSION"] != "2026-06-01" {
		t.Errorf("drweb = %v", av)
	}
}

func TestParseCortex(t *testing.T) {
	av := ParseCortex("Cortex XDR Agent 8.4.0.51060\n", "Content Version:\t1240-98765\n")
	if av["VERSION"] != "8.4.0.51060" || av["BASE_VERSION"] != "1240-98765" {
		t.Errorf("cortex = %v", av)
	}
}

func TestParseEEA(t *testing.T) {
	av := ParseEEA("Version: (eea) 10.0.2.0\n", "active\n",
		"License Validity: 2027-01-31\n", "EM002 1234 (20240101) Detection engine\n")
	if av["VERSION"] != "10.0.2.0" || av["ENABLED"] != 1 {
		t.Errorf("eea = %v", av)
	}
	if av["EXPIRATION"] != "2027-01-31" || av["BASE_VERSION"] != "1234 (20240101)" {
		t.Errorf("eea detail = %v", av)
	}
}

func TestParseKESL(t *testing.T) {
	const appInfo = `Version:                      12.0.0.5littleendian
Application running:          Yes
License expiration date:      2027-03-15
Last release date of databases: 2026-06-06`
	av := ParseKESL("active\n", appInfo)
	if av["ENABLED"] != 1 || av["VERSION"] != "12.0.0.5" {
		t.Errorf("kesl = %v", av)
	}
	if av["EXPIRATION"] != "2027-03-15" || av["BASE_VERSION"] != "2026-06-06" {
		t.Errorf("kesl detail = %v", av)
	}
}
