// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
	"testing"
)

func loadSysctl(t *testing.T, name string) map[string]string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "macos", "sysctl", name))
	if err != nil {
		t.Fatalf("read sysctl %s: %v", name, err)
	}
	return parseSysctl(string(data))
}

// TestBuildMacCPUs pins the CPU mapper against the real system_profiler + sysctl
// captures, using the expected values from t/tasks/inventory/macos/cpu.t.
func TestBuildMacCPUs(t *testing.T) {
	// 10.6 Mac mini (Intel Core 2 Duo).
	hw := loadSP(t, "10.6-macmini")
	overview := spNode(hw, "Hardware", "Hardware Overview")
	cpus := buildMacCPUs(overview, loadSysctl(t, "10.6-macmini"))
	if len(cpus) != 1 {
		t.Fatalf("macmini: got %d cpus, want 1", len(cpus))
	}
	want := map[string]any{
		"CORE":         "2",
		"MANUFACTURER": "Intel",
		"NAME":         "Intel(R) Core(TM)2 Duo CPU P7550 @ 2.26GHz",
		"THREAD":       "2",
		"FAMILYNUMBER": "6",
		"MODEL":        "23",
		"STEPPING":     "10",
		"SPEED":        2260,
	}
	for k, v := range want {
		if cpus[0][k] != v {
			t.Errorf("macmini cpu[%s] = %v, want %v", k, cpus[0][k], v)
		}
	}

	// 11.0 Apple M1.
	hwM1 := loadSP(t, "11.0-apple-M1")
	cpusM1 := buildMacCPUs(spNode(hwM1, "Hardware", "Hardware Overview"), loadSysctl(t, "11.0-apple-M1"))
	if len(cpusM1) != 1 {
		t.Fatalf("M1: got %d cpus, want 1", len(cpusM1))
	}
	m1 := cpusM1[0]
	if m1["CORE"] != "8" || m1["MANUFACTURER"] != "Apple" || m1["NAME"] != "Apple M1" || m1["THREAD"] != "8" {
		t.Errorf("M1 cpu = %v", m1)
	}
	if _, ok := m1["SPEED"]; ok {
		t.Errorf("M1 should have no SPEED, got %v", m1["SPEED"])
	}
	if _, ok := m1["FAMILYNUMBER"]; ok {
		t.Errorf("M1 should have no FAMILYNUMBER")
	}
}

// TestMacCPUSpeed covers the speed normalisation.
func TestMacCPUSpeed(t *testing.T) {
	cases := map[string]int{
		"2,26 GHz": 2260,
		"2.26 GHz": 2260,
		"800 MHz":  800,
		"":         0,
	}
	for in, want := range cases {
		if got := macCPUSpeed(in); got != want {
			t.Errorf("macCPUSpeed(%q) = %d, want %d", in, got, want)
		}
	}
}
