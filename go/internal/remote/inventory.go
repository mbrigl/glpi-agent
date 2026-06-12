// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"strings"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/content"
	"github.com/glpi-project/glpi-agent/go/internal/inventory"
)

// remoteSystem is the host access a remote inventory needs: run commands, read
// files and probe binaries. It is the Go counterpart of the upstream
// GLPI::Agent::Tools::remote seam; SSHClient implements it (WinRM later).
type remoteSystem interface {
	Run(command string) (string, error)
	ReadFile(path string) (string, error)
	CanRun(binary string) bool
	OSName() (string, error)
	Hostname(fallback string) string
	FQDN() string
}

// rpmQueryFormat mirrors the format the local RPM collector uses
// (Generic/Softwares/RPM.pm).
const rpmQueryFormat = `%{NAME}\t%{ARCH}\t%{VERSION}-%{RELEASE}\t%{INSTALLTIME}\t%{SIZE}\t%{VENDOR}\t%{SUMMARY}\t%{GROUP}\n`

// CollectInventory builds an inventory for a remote host over SSH, mirroring the
// remote path of GLPI::Agent::Task::RemoteInventory: the local inventory commands
// are run over the remote executor and parsed with the shared (Phase 6) pure
// parsers.
func (c *SSHClient) CollectInventory(itemtype, tag, fallbackHost string) (*content.Inventory, error) {
	return buildRemoteInventory(c, itemtype, tag, fallbackHost), nil
}

// buildRemoteInventory assembles the remote inventory from a remoteSystem. It
// reuses the exported pure parsers from internal/inventory by feeding them the
// output of the equivalent remote command / file read. The sysfs-only Linux
// collectors (networks, drives, batteries, …) still need the full filesystem
// abstraction and are follow-on.
func buildRemoteInventory(sys remoteSystem, itemtype, tag, fallbackHost string) *content.Inventory {
	hostname := sys.Hostname(fallbackHost)
	inv := content.New(content.DeviceID(hostname, time.Now()))
	if itemtype != "" {
		inv.ItemType = itemtype
	}
	if tag != "" {
		inv.Content["ACCOUNTINFO"] = map[string]any{"KEYNAME": "TAG", "KEYVALUE": tag}
	}

	hardware := inv.Content["HARDWARE"].(map[string]any)
	hardware["NAME"] = hostname

	// OPERATINGSYSTEM: kernel (uname) + os-release distro fields + FQDN.
	os := map[string]any{}
	if osname, err := sys.OSName(); err == nil && osname != "" {
		os["KERNEL_NAME"] = osname
	}
	if version, err := sys.Run("uname -r"); err == nil {
		if v := firstLineOf(version); v != "" {
			os["KERNEL_VERSION"] = v
		}
	}
	if arch, err := sys.Run("uname -m"); err == nil {
		if a := firstLineOf(arch); a != "" {
			hardware["ARCH"] = a
		}
	}
	if rel, err := sys.ReadFile("/etc/os-release"); err == nil && rel != "" {
		for k, v := range inventory.ParseOSRelease(strings.NewReader(rel)) {
			os[k] = v
		}
	}
	if fqdn := sys.FQDN(); fqdn != "" {
		os["FQDN"] = fqdn
	}
	if len(os) > 0 {
		inv.Content["OPERATINGSYSTEM"] = os
	}

	// HARDWARE memory/swap from /proc/meminfo.
	if mem, err := sys.ReadFile("/proc/meminfo"); err == nil && mem != "" {
		if m, swap := inventory.ParseMemInfo(strings.NewReader(mem)); m > 0 || swap > 0 {
			if m > 0 {
				hardware["MEMORY"] = m
			}
			if swap > 0 {
				hardware["SWAP"] = swap
			}
		}
	}

	// CPUS from /proc/cpuinfo.
	if cpuinfo, err := sys.ReadFile("/proc/cpuinfo"); err == nil && cpuinfo != "" {
		if cpus := inventory.ParseCPUInfo(strings.NewReader(cpuinfo)); len(cpus) > 0 {
			inv.Content["CPUS"] = cpus
		}
	}

	// MEMORIES / SLOTS / PORTS from dmidecode (needs privileges remotely).
	if sys.CanRun("dmidecode") {
		if out, err := sys.Run("dmidecode"); err == nil && out != "" {
			byType := inventory.ParseDmidecode(strings.NewReader(out))
			setIfAny(inv, "MEMORIES", inventory.BuildMemories(byType))
			setIfAny(inv, "SLOTS", inventory.BuildSlots(byType))
			setIfAny(inv, "PORTS", inventory.BuildPorts(byType))
		}
	}

	collectRemoteSoftwares(sys, inv)
	collectRemoteLVM(sys, inv)

	// sysfs-based sections (BATTERIES, USBDEVICES, STORAGES) via the filesystem
	// abstraction reading the remote host's /sys over SSH.
	for section, entries := range inventory.CollectFileSectionsFS(remoteFS{sys: sys}) {
		inv.Content[section] = entries
	}

	return inv
}

// remoteFS adapts a remoteSystem to inventory.FS: file reads via the remote
// `cat`, globs expanded by the remote shell.
type remoteFS struct{ sys remoteSystem }

func (f remoteFS) ReadFile(path string) ([]byte, error) {
	out, err := f.sys.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return []byte(out), nil
}

func (f remoteFS) Glob(pattern string) ([]string, error) {
	out, err := f.sys.Run("ls -d " + pattern + " 2>/dev/null")
	if err != nil {
		return nil, nil
	}
	var matches []string
	for _, line := range strings.Split(out, "\n") {
		if l := strings.TrimSpace(line); l != "" {
			matches = append(matches, l)
		}
	}
	return matches, nil
}

// collectRemoteSoftwares fills SOFTWARES from rpm, falling back to the dpkg
// status file.
func collectRemoteSoftwares(sys remoteSystem, inv *content.Inventory) {
	if sys.CanRun("rpm") {
		if out, err := sys.Run("rpm -qa --queryformat '" + rpmQueryFormat + "'"); err == nil && out != "" {
			setIfAny(inv, "SOFTWARES", inventory.ParseRPMQA(out))
			return
		}
	}
	if status, err := sys.ReadFile("/var/lib/dpkg/status"); err == nil && status != "" {
		setIfAny(inv, "SOFTWARES", inventory.ParseDpkgStatus(strings.NewReader(status)))
	}
}

// collectRemoteLVM fills the LVM sections from pvs/vgs/lvs.
func collectRemoteLVM(sys remoteSystem, inv *content.Inventory) {
	if !sys.CanRun("lvs") {
		return
	}
	if out, err := sys.Run("pvs --noheading --nosuffix --units M -o pv_name,pv_fmt,pv_attr,pv_size,pv_free,pv_uuid,pv_pe_count,vg_uuid"); err == nil {
		setIfAny(inv, "PHYSICAL_VOLUMES", inventory.ParsePVS(out))
	}
	if out, err := sys.Run("vgs --noheading --nosuffix --units M -o vg_name,pv_count,lv_count,vg_attr,vg_size,vg_free,vg_uuid,vg_extent_size"); err == nil {
		setIfAny(inv, "VOLUME_GROUPS", inventory.ParseVGS(out))
	}
	if out, err := sys.Run("lvs -a --noheading --nosuffix --units M -o lv_name,vg_uuid,lv_attr,lv_size,lv_uuid,seg_count"); err == nil {
		setIfAny(inv, "LOGICAL_VOLUMES", inventory.ParseLVS(out))
	}
}

// setIfAny stores a section only when it has entries.
func setIfAny(inv *content.Inventory, section string, entries []map[string]any) {
	if len(entries) > 0 {
		inv.Content[section] = entries
	}
}

// firstLineOf returns the first non-empty trimmed line of s.
func firstLineOf(s string) string {
	for _, line := range strings.Split(s, "\n") {
		if t := strings.TrimSpace(line); t != "" {
			return t
		}
	}
	return ""
}
