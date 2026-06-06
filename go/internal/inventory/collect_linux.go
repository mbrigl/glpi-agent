// SPDX-License-Identifier: GPL-2.0-only

//go:build linux

package inventory

import (
	"net"
	"os"
	"strconv"
	"strings"
	"syscall"
)

// Collect gathers the local Linux inventory: OPERATINGSYSTEM, HARDWARE (name +
// memory) and CPUS. It reads the same sources as the upstream Perl modules
// (/etc/os-release, /proc/sys/kernel/osrelease, /proc/meminfo, /proc/cpuinfo).
func Collect() Sections {
	s := Sections{}

	// OPERATINGSYSTEM: os-release distro fields + kernel name/version.
	os := map[string]any{"KERNEL_NAME": "linux"}
	if f, err := osOpen("/etc/os-release"); err == nil {
		for k, v := range ParseOSRelease(f) {
			os[k] = v
		}
		f.Close()
	}
	if rel := firstLine("/proc/sys/kernel/osrelease"); rel != "" {
		os["KERNEL_VERSION"] = rel
	}
	s["OPERATINGSYSTEM"] = os

	// HARDWARE: hostname + memory/swap.
	s.mergeHardware(map[string]any{"NAME": hostname()})
	if f, err := osOpen("/proc/meminfo"); err == nil {
		mem, swap := ParseMemInfo(f)
		f.Close()
		hw := map[string]any{}
		if mem > 0 {
			hw["MEMORY"] = mem
		}
		if swap > 0 {
			hw["SWAP"] = swap
		}
		s.mergeHardware(hw)
	}

	// CPUS.
	if f, err := osOpen("/proc/cpuinfo"); err == nil {
		if cpus := ParseCPUInfo(f); len(cpus) > 0 {
			s["CPUS"] = cpus
		}
		f.Close()
	}

	// BIOS from the DMI sysfs tree.
	if bios := ParseDMI(readDMI()); len(bios) > 0 {
		s["BIOS"] = bios
	}

	// NETWORKS from the kernel interface list + sysfs details.
	if nets := BuildNetworks(linuxInterfaces()); len(nets) > 0 {
		s["NETWORKS"] = nets
	}

	// DRIVES (mounted filesystems) from /proc/mounts + statfs.
	if f, err := osOpen("/proc/mounts"); err == nil {
		mounts := ParseMounts(f)
		f.Close()
		if drives := BuildDrives(mounts, statfsMB); len(drives) > 0 {
			s["DRIVES"] = drives
		}
	}

	// SOFTWARES from the dpkg status database (Debian/Ubuntu).
	if f, err := osOpen("/var/lib/dpkg/status"); err == nil {
		if sw := ParseDpkgStatus(f); len(sw) > 0 {
			s["SOFTWARES"] = sw
		}
		f.Close()
	}

	return s
}

// statfsMB returns a mountpoint's total and free space in MiB via statfs(2).
func statfsMB(mountpoint string) (totalMB, freeMB int, ok bool) {
	var st syscall.Statfs_t
	if err := syscall.Statfs(mountpoint, &st); err != nil {
		return 0, 0, false
	}
	bs := uint64(st.Bsize)
	const mib = 1024 * 1024
	return int(st.Blocks * bs / mib), int(st.Bavail * bs / mib), true
}

// readDMI reads the /sys/class/dmi/id fields ParseDMI consumes.
func readDMI() map[string]string {
	const base = "/sys/class/dmi/id"
	fields := []string{
		"bios_vendor", "bios_version", "bios_date", "sys_vendor", "product_name",
		"product_sku", "product_serial", "board_vendor", "board_name",
		"board_serial", "chassis_asset_tag", "chassis_serial",
	}
	dmi := map[string]string{}
	for _, f := range fields {
		if v := firstLine(base + "/" + f); v != "" {
			dmi[f] = v
		}
	}
	return dmi
}

// linuxInterfaces builds the NetIface list from the kernel interface table and
// per-interface sysfs attributes (virtual flag, link speed, driver).
func linuxInterfaces() []NetIface {
	ifaces, err := net.Interfaces()
	if err != nil {
		return nil
	}
	var out []NetIface
	for _, iface := range ifaces {
		ni := NetIface{
			Name: iface.Name,
			MAC:  iface.HardwareAddr.String(),
			Up:   iface.Flags&net.FlagUp != 0,
		}
		if _, err := os.Stat("/sys/devices/virtual/net/" + iface.Name); err == nil {
			ni.Virtual = true
		}
		if sp := firstLine("/sys/class/net/" + iface.Name + "/speed"); sp != "" {
			if n, err := strconv.Atoi(sp); err == nil && n > 0 {
				ni.Speed = n
			}
		}
		ni.Driver = ueventDriver(iface.Name)

		addrs, _ := iface.Addrs()
		for _, a := range addrs {
			if ipnet, ok := a.(*net.IPNet); ok && ipnet.IP.To4() != nil {
				ni.Addrs = append(ni.Addrs, NetAddr{
					IP:   ipnet.IP.String(),
					Mask: net.IP(ipnet.Mask).String(),
				})
			}
		}
		out = append(out, ni)
	}
	return out
}

// ueventDriver extracts DRIVER= from /sys/class/net/<name>/device/uevent
// (Linux/Networks.pm _getUevent).
func ueventDriver(name string) string {
	data, err := os.ReadFile("/sys/class/net/" + name + "/device/uevent")
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(data), "\n") {
		if v, ok := strings.CutPrefix(line, "DRIVER="); ok {
			return strings.TrimSpace(v)
		}
	}
	return ""
}

func osOpen(path string) (*os.File, error) { return os.Open(path) }

func firstLine(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(strings.SplitN(string(data), "\n", 2)[0])
}
