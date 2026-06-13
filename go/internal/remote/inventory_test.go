// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"errors"
	"testing"
)

// fakeSystem is a scripted remoteSystem for the inventory build test.
type fakeSystem struct {
	commands map[string]string
	files    map[string]string
	runnable map[string]bool
	osname   string
	host     string
	fqdn     string
}

func (f *fakeSystem) Run(command string) (string, error) {
	if out, ok := f.commands[command]; ok {
		return out, nil
	}
	return "", nil
}
func (f *fakeSystem) ReadFile(path string) (string, error) {
	if out, ok := f.files[path]; ok {
		return out, nil
	}
	return "", errors.New("no such file")
}
func (f *fakeSystem) CanRun(binary string) bool { return f.runnable[binary] }
func (f *fakeSystem) OSName() (string, error)   { return f.osname, nil }
func (f *fakeSystem) Hostname(fallback string) string {
	if f.host != "" {
		return f.host
	}
	return fallback
}
func (f *fakeSystem) FQDN() string { return f.fqdn }

// TestBuildRemoteInventory checks the remote inventory reuses the shared pure
// parsers: OS-release, /proc/cpuinfo, /proc/meminfo, and rpm softwares.
func TestBuildRemoteInventory(t *testing.T) {
	sys := &fakeSystem{
		osname: "linux",
		host:   "remote-host",
		fqdn:   "remote-host.example.com",
		commands: map[string]string{
			"uname -r": "5.15.0-generic\n",
			"uname -m": "x86_64\n",
			"rpm -qa --queryformat '" + rpmQueryFormat + "'": "bash\tx86_64\t5.1-2\t1600000000\t1048576\tRed Hat\tThe GNU Bourne Again shell\tSystem\n",
		},
		files: map[string]string{
			"/etc/os-release": `NAME="Fedora Linux"
VERSION="38 (Workstation Edition)"
PRETTY_NAME="Fedora Linux 38 (Workstation Edition)"
`,
			"/proc/meminfo": "MemTotal:        2097152 kB\nSwapTotal:       1048576 kB\n",
			"/proc/cpuinfo": "processor\t: 0\nmodel name\t: Intel(R) Core(TM) i5\nvendor_id\t: GenuineIntel\n",
		},
		runnable: map[string]bool{"rpm": true},
	}

	inv := buildRemoteInventory(sys, "Computer", "lab", "fallback")

	if inv.ItemType != "Computer" {
		t.Errorf("itemtype = %q", inv.ItemType)
	}
	hw := inv.Content["HARDWARE"].(map[string]any)
	if hw["NAME"] != "remote-host" || hw["ARCH"] != "x86_64" || hw["MEMORY"] != 2048 || hw["SWAP"] != 1024 {
		t.Errorf("hardware = %v", hw)
	}
	os := inv.Content["OPERATINGSYSTEM"].(map[string]any)
	if os["KERNEL_NAME"] != "linux" || os["KERNEL_VERSION"] != "5.15.0-generic" ||
		os["FULL_NAME"] != "Fedora Linux 38 (Workstation Edition)" || os["FQDN"] != "remote-host.example.com" {
		t.Errorf("os = %v", os)
	}
	cpus, _ := inv.Content["CPUS"].([]map[string]any)
	if len(cpus) == 0 {
		t.Errorf("no CPUS collected")
	}
	sw, _ := inv.Content["SOFTWARES"].([]map[string]any)
	if len(sw) != 1 || sw[0]["NAME"] != "bash" {
		t.Errorf("softwares = %v", sw)
	}
	if _, ok := inv.Content["ACCOUNTINFO"]; !ok {
		t.Errorf("missing tag ACCOUNTINFO")
	}
}

// TestBuildRemoteInventorySysfs checks a sysfs-based section (BATTERIES) is
// collected over SSH through the filesystem abstraction (glob + cat).
func TestBuildRemoteInventorySysfs(t *testing.T) {
	sys := &fakeSystem{
		host: "laptop",
		commands: map[string]string{
			"ls -d /sys/class/power_supply/* 2>/dev/null": "/sys/class/power_supply/BAT0\n",
		},
		files: map[string]string{
			"/sys/class/power_supply/BAT0/type":               "Battery",
			"/sys/class/power_supply/BAT0/present":            "1",
			"/sys/class/power_supply/BAT0/capacity":           "95",
			"/sys/class/power_supply/BAT0/model_name":         "DELL ABC123",
			"/sys/class/power_supply/BAT0/technology":         "Li-ion",
			"/sys/class/power_supply/BAT0/voltage_min_design": "11400000",
			"/sys/class/power_supply/BAT0/energy_full_design": "50000000",
		},
	}
	inv := buildRemoteInventory(sys, "", "", "laptop")
	bats, _ := inv.Content["BATTERIES"].([]map[string]any)
	if len(bats) != 1 {
		t.Fatalf("got %d batteries, want 1", len(bats))
	}
	b := bats[0]
	if b["NAME"] != "DELL ABC123" || b["CHEMISTRY"] != "Li-ion" || b["VOLTAGE"] != 11400 || b["CAPACITY"] != 50000 {
		t.Errorf("remote battery = %v", b)
	}
}

// TestBuildRemoteInventoryCommandSections checks the BIOS (DMI), LOCAL_USERS and
// DRIVES sections collected over SSH via the shared parsers.
func TestBuildRemoteInventoryCommandSections(t *testing.T) {
	sys := &fakeSystem{
		host: "srv",
		commands: map[string]string{
			"df -P -k '/'": "Filesystem 1024-blocks    Used Available Capacity Mounted on\n/dev/sda1   10485760 1048576   9437184      10% /\n",
		},
		files: map[string]string{
			"/sys/class/dmi/id/bios_vendor":  "American Megatrends Inc.",
			"/sys/class/dmi/id/bios_version": "F4",
			"/sys/class/dmi/id/sys_vendor":   "Gigabyte",
			"/etc/passwd":                    "root:x:0:0:root:/root:/bin/bash\njdoe:x:1000:1000:J Doe:/home/jdoe:/bin/bash\n",
			"/etc/group":                     "root:x:0:\nsudo:x:27:jdoe\n",
			"/proc/mounts":                   "/dev/sda1 / ext4 rw,relatime 0 0\n",
		},
	}
	inv := buildRemoteInventory(sys, "", "", "srv")

	bios, _ := inv.Content["BIOS"].(map[string]any)
	if bios["BMANUFACTURER"] != "American Megatrends Inc." || bios["BVERSION"] != "F4" {
		t.Errorf("bios = %v", bios)
	}
	users, _ := inv.Content["LOCAL_USERS"].([]map[string]any)
	if len(users) != 2 {
		t.Errorf("local users = %v", users)
	}
	drives, _ := inv.Content["DRIVES"].([]map[string]any)
	if len(drives) != 1 || drives[0]["VOLUMN"] != "/dev/sda1" || drives[0]["TOTAL"] != 10240 || drives[0]["FREE"] != 9216 {
		t.Errorf("drives = %v", drives)
	}
}

// TestParseDfStatfs covers the df usage parser.
func TestParseDfStatfs(t *testing.T) {
	total, free, ok := parseDfStatfs("Filesystem 1024-blocks Used Available Capacity Mounted\n/dev/x 2097152 1048576 1048576 50% /m\n")
	if !ok || total != 2048 || free != 1024 {
		t.Errorf("parseDfStatfs = %d/%d/%v", total, free, ok)
	}
}

// TestBuildRemoteInventoryDpkgFallback checks the dpkg status fallback when rpm
// is absent.
func TestBuildRemoteInventoryDpkgFallback(t *testing.T) {
	sys := &fakeSystem{
		osname: "linux",
		host:   "deb-host",
		files: map[string]string{
			"/var/lib/dpkg/status": "Package: coreutils\nStatus: install ok installed\nVersion: 8.32-4\nArchitecture: amd64\n\n",
		},
	}
	inv := buildRemoteInventory(sys, "", "", "fallback")
	sw, _ := inv.Content["SOFTWARES"].([]map[string]any)
	if len(sw) != 1 || sw[0]["NAME"] != "coreutils" {
		t.Errorf("dpkg softwares = %v", sw)
	}
}
