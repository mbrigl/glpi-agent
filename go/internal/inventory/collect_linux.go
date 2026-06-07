// SPDX-License-Identifier: GPL-2.0-only

//go:build linux

package inventory

import (
	"net"
	"os"
	"os/exec"
	"path/filepath"
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
	osInfo := map[string]any{"KERNEL_NAME": "linux"}
	if f, err := osOpen("/etc/os-release"); err == nil {
		for k, v := range ParseOSRelease(f) {
			osInfo[k] = v
		}
		f.Close()
	}
	if rel := firstLine("/proc/sys/kernel/osrelease"); rel != "" {
		osInfo["KERNEL_VERSION"] = rel
	}
	s["OPERATINGSYSTEM"] = osInfo

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

	// SOFTWARES: dpkg (Debian/Ubuntu) or rpm (RHEL/Fedora/SUSE).
	if f, err := osOpen("/var/lib/dpkg/status"); err == nil {
		if sw := ParseDpkgStatus(f); len(sw) > 0 {
			s["SOFTWARES"] = sw
		}
		f.Close()
	} else if _, err := exec.LookPath("rpm"); err == nil {
		// rpm interprets the \t and \n escapes in the queryformat itself.
		if sw := ParseRPMQA(runCommand("rpm", "-qa", "--queryformat", rpmQueryFormat)); len(sw) > 0 {
			s["SOFTWARES"] = sw
		}
	}

	// LOCAL_USERS / LOCAL_GROUPS from /etc/passwd + /etc/group.
	passwd, _ := os.Open("/etc/passwd")
	group, _ := os.Open("/etc/group")
	users, groups := BuildUsers(passwd, group)
	if passwd != nil {
		passwd.Close()
	}
	if group != nil {
		group.Close()
	}
	if len(users) > 0 {
		s["LOCAL_USERS"] = users
	}
	if len(groups) > 0 {
		s["LOCAL_GROUPS"] = groups
	}

	// ENVS from the process environment.
	if envs := BuildEnvs(osEnviron()); len(envs) > 0 {
		s["ENVS"] = envs
	}

	// STORAGES and BATTERIES from sysfs.
	if st := BuildStorages(""); len(st) > 0 {
		s["STORAGES"] = st
	}
	if bat := BuildBatteries(""); len(bat) > 0 {
		s["BATTERIES"] = bat
	}

	// INPUTS from /proc/bus/input/devices.
	if f, err := osOpen("/proc/bus/input/devices"); err == nil {
		if in := ParseInputDevices(f); len(in) > 0 {
			s["INPUTS"] = in
		}
		f.Close()
	}

	// USBDEVICES from sysfs.
	if usb := BuildUSB(""); len(usb) > 0 {
		s["USBDEVICES"] = usb
	}

	// MEMORIES / SLOTS / PORTS from one dmidecode scan (best-effort: needs the
	// dmidecode tool and privileges).
	if out := runCommand("dmidecode"); out != "" {
		dmi := ParseDmidecode(strings.NewReader(out))
		if mem := BuildMemories(dmi); len(mem) > 0 {
			s["MEMORIES"] = mem
		}
		if slots := BuildSlots(dmi); len(slots) > 0 {
			s["SLOTS"] = slots
		}
		if ports := BuildPorts(dmi); len(ports) > 0 {
			s["PORTS"] = ports
		}
	}

	// PROCESSES from /proc.
	if procs := collectProcesses(); len(procs) > 0 {
		s["PROCESSES"] = procs
	}

	// USERS: currently logged-in users (who).
	if _, err := exec.LookPath("who"); err == nil {
		if u := BuildLoggedUsers(runCommand("who", "--users")); len(u) > 0 {
			s["USERS"] = u
		}
	}

	// PRINTERS from the CUPS configuration.
	if f, err := osOpen("/etc/cups/printers.conf"); err == nil {
		if p := ParsePrintersConf(f); len(p) > 0 {
			s["PRINTERS"] = p
		}
		f.Close()
	}

	// FIREWALL: ufw and/or firewalld status.
	if fw := collectFirewall(); len(fw) > 0 {
		s["FIREWALL"] = fw
	}

	// ANTIVIRUS: product-specific detectors (Defender, CrowdStrike, …).
	if av := collectAntivirus(); len(av) > 0 {
		s["ANTIVIRUS"] = av
	}

	// REMOTE_MGMT: remote-control agents (TeamViewer, AnyDesk, RustDesk, …).
	if rm := collectRemoteMgmt(); len(rm) > 0 {
		s["REMOTE_MGMT"] = rm
	}

	// VIRTUALMACHINES: local guests across the available hypervisors.
	if vms := collectVirtualMachines(); len(vms) > 0 {
		s["VIRTUALMACHINES"] = vms
	}

	// MONITORS from the DRM EDID blocks.
	if mon := collectMonitors(); len(mon) > 0 {
		s["MONITORS"] = mon
	}

	// LVM: PHYSICAL_VOLUMES / VOLUME_GROUPS / LOGICAL_VOLUMES (needs the lvm2
	// tools and privileges).
	if _, err := exec.LookPath("lvs"); err == nil {
		if pv := ParsePVS(runCommand("pvs", "--noheading", "--nosuffix", "--units", "M",
			"-o", "pv_name,pv_fmt,pv_attr,pv_size,pv_free,pv_uuid,pv_pe_count,vg_uuid")); len(pv) > 0 {
			s["PHYSICAL_VOLUMES"] = pv
		}
		if vg := ParseVGS(runCommand("vgs", "--noheading", "--nosuffix", "--units", "M",
			"-o", "vg_name,pv_count,lv_count,vg_attr,vg_size,vg_free,vg_uuid,vg_extent_size")); len(vg) > 0 {
			s["VOLUME_GROUPS"] = vg
		}
		if lv := ParseLVS(runCommand("lvs", "-a", "--noheading", "--nosuffix", "--units", "M",
			"-o", "lv_name,vg_uuid,lv_attr,lv_size,lv_uuid,seg_count")); len(lv) > 0 {
			s["LOGICAL_VOLUMES"] = lv
		}
	}

	// CONTROLLERS / VIDEOS / SOUNDS from one lspci scan.
	if out := runCommand("lspci", "-v", "-nn"); out != "" {
		devices := ParseLspci(strings.NewReader(out))
		if c := BuildControllers(devices); len(c) > 0 {
			s["CONTROLLERS"] = c
		}
		if v := BuildVideos(devices); len(v) > 0 {
			s["VIDEOS"] = v
		}
		if snd := BuildSounds(devices); len(snd) > 0 {
			s["SOUNDS"] = snd
		}
	}

	return s
}

// collectVirtualMachines gathers local guests from every available hypervisor,
// mirroring the per-hypervisor Virtualization/* modules.
func collectVirtualMachines() []map[string]any {
	var vms []map[string]any
	vms = append(vms, collectLibvirt()...)
	vms = append(vms, collectDocker()...)
	vms = append(vms, collectVirtualBox()...)
	vms = append(vms, collectNspawn()...)
	vms = append(vms, collectXen()...)
	vms = append(vms, collectVirtuozzo()...)
	return vms
}

// collectXen lists Xen domains via xl (or the legacy xm), mirroring
// Virtualization/Xen.pm.
func collectXen() []map[string]any {
	for _, tool := range []string{"xl", "xm"} {
		if _, err := exec.LookPath(tool); err == nil {
			return ParseXenList(runCommand(tool, "list"), tool)
		}
	}
	return nil
}

// collectVirtuozzo lists OpenVZ/Virtuozzo containers, mirroring
// Virtualization/Virtuozzo.pm.
func collectVirtuozzo() []map[string]any {
	if _, err := exec.LookPath("vzlist"); err != nil {
		return nil
	}
	return ParseVirtuozzo(runCommand("vzlist", "--all", "--no-header",
		"-o", "hostname,ctid,cpulimit,status,ostemplate"))
}

// collectNspawn lists systemd-nspawn machines via machinectl, mirroring
// Virtualization/SystemdNspawn.pm.
func collectNspawn() []map[string]any {
	if _, err := exec.LookPath("machinectl"); err != nil {
		return nil
	}
	return ParseMachinectl(runCommand("machinectl", "--no-pager", "--no-legend"))
}

// collectDocker lists containers and resolves each one's running state, mirroring
// Virtualization/Docker.pm (needs docker).
func collectDocker() []map[string]any {
	if _, err := exec.LookPath("docker"); err != nil {
		return nil
	}
	containers := ParseDockerPS(runCommand("docker", "ps", "-a", "--format", DockerPSTemplate))
	for _, c := range containers {
		id, _ := c["UUID"].(string)
		running := strings.TrimSpace(runCommand("docker", "inspect", "--format", "{{.State.Running}}", id))
		if running == "true" {
			c["STATUS"] = "running"
		} else {
			c["STATUS"] = "off"
		}
	}
	return containers
}

// collectVirtualBox lists VMs and parses each one's showvminfo, mirroring
// Virtualization/VirtualBox.pm (needs VBoxManage).
func collectVirtualBox() []map[string]any {
	if _, err := exec.LookPath("VBoxManage"); err != nil {
		return nil
	}
	var dump strings.Builder
	for _, uuid := range ParseVBoxList(runCommand("VBoxManage", "-nologo", "list", "vms")) {
		dump.WriteString(runCommand("VBoxManage", "-nologo", "showvminfo", uuid))
		dump.WriteByte('\n')
	}
	return ParseVBoxShowVMInfo(dump.String())
}

// collectLibvirt lists local libvirt guests and enriches each from its domain
// XML, mirroring Virtualization/Libvirt.pm (needs virsh).
func collectLibvirt() []map[string]any {
	if _, err := exec.LookPath("virsh"); err != nil {
		return nil
	}
	machines := ParseVirshList(runCommand("virsh", "--readonly", "list", "--all"))
	for _, m := range machines {
		name, _ := m["NAME"].(string)
		if name == "" {
			continue
		}
		ApplyVirshDumpXML(m, runCommand("virsh", "--readonly", "dumpxml", name))
	}
	return machines
}

// collectAntivirus gathers product-specific antivirus entries, mirroring the
// per-product Linux/AntiVirus/* detectors.
func collectAntivirus() []map[string]any {
	var av []map[string]any
	add := func(e map[string]any) {
		if e != nil {
			av = append(av, e)
		}
	}

	if _, err := exec.LookPath("mdatp"); err == nil {
		add(ParseDefenderHealth([]byte(runCommand("mdatp", "health", "--output", "json"))))
	}
	if hasFile("/opt/CrowdStrike/falconctl") {
		add(ParseCrowdStrikeVersion(runCommand("/opt/CrowdStrike/falconctl", "-g", "--version")))
	}
	if bdui := "/opt/bitdefender-security-tools/bin/bduitool"; hasFile(bdui) {
		add(ParseBitdefender(runCommand(bdui, "get", "ps")))
	}
	if cytool := "/opt/traps/bin/cytool"; hasFile(cytool) {
		add(ParseCortex(runCommand(cytool, "info"), runCommand(cytool, "info", "query")))
	}
	if _, err := exec.LookPath("drweb-ctl"); err == nil {
		add(ParseDrWeb(
			runCommand("drweb-ctl", "--version"),
			runCommand("systemctl", "is-active", "drweb-configd.service"),
			runCommand("drweb-ctl", "baseinfo"),
		))
	}
	if upd, lic := "/opt/eset/eea/bin/upd", "/opt/eset/eea/sbin/lic"; hasFile(upd) && hasFile(lic) {
		add(ParseEEA(
			runCommand(upd, "-version"),
			runCommand("systemctl", "is-active", "eea.service"),
			runCommand(lic, "--status"),
			runCommand(upd, "--list-modules"),
		))
	}
	if _, err := exec.LookPath("kesl-control"); err == nil {
		add(ParseKESL(
			runCommand("systemctl", "is-active", "kesl.service"),
			runCommand("kesl-control", "--app-info"),
		))
	}
	if sc := "/opt/sentinelone/bin/sentinelctl"; hasFile(sc) {
		add(ParseSentinelOne(runCommand(sc, "version") + "\n" +
			runCommand(sc, "engines", "status") + "\n" +
			runCommand(sc, "control", "status") + "\n" +
			runCommand(sc, "management", "status")))
	}
	return av
}

// hasFile reports whether a path exists.
func hasFile(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// collectRemoteMgmt gathers remote-management agent ids, mirroring the
// per-agent Generic/Remote_Mgmt/* detectors.
func collectRemoteMgmt() []map[string]any {
	var rm []map[string]any
	if _, err := exec.LookPath("teamviewer"); err == nil {
		if e := ParseTeamViewerInfo(runCommand("teamviewer", "--info")); e != nil {
			rm = append(rm, e)
		}
	}
	for _, conf := range append(globMatches("/etc/anydesk_ad_*/system.conf"), "/etc/anydesk/system.conf") {
		if data, err := os.ReadFile(conf); err == nil {
			if e := ParseAnyDeskID(string(data)); e != nil {
				rm = append(rm, e)
				break
			}
		}
	}
	if data, err := os.ReadFile("/root/.config/rustdesk/RustDesk.toml"); err == nil {
		if e := ParseRustDeskID(string(data)); e != nil {
			rm = append(rm, e)
		}
	}
	return rm
}

// globMatches returns the glob matches for a pattern (empty on error).
func globMatches(pattern string) []string {
	m, _ := filepath.Glob(pattern)
	return m
}

// collectFirewall reports the ufw and firewalld status as FIREWALL entries,
// mirroring Generic/Firewall/{Ufw,Systemd}.pm (STATUS on/off).
func collectFirewall() []map[string]any {
	var fw []map[string]any
	if _, err := exec.LookPath("ufw"); err == nil {
		status := "off"
		if strings.Contains(runCommand("ufw", "status"), "Status: active") {
			status = "on"
		}
		fw = append(fw, map[string]any{"DESCRIPTION": "ufw", "STATUS": status})
	}
	if _, err := exec.LookPath("systemctl"); err == nil {
		if strings.TrimSpace(runCommand("systemctl", "is-active", "firewalld")) == "active" {
			fw = append(fw, map[string]any{"DESCRIPTION": "firewalld", "STATUS": "on"})
		}
	}
	return fw
}

// collectMonitors reads the EDID block of each connected DRM output and builds
// the MONITORS entries (Generic/Screen.pm reads the same drm card edid files).
// Entries are de-duplicated by serial/BASE64.
func collectMonitors() []map[string]any {
	matches, _ := filepathGlob(
		"/sys/devices/*/*/drm/*/edid",
		"/sys/devices/*/*/*/drm/*/edid",
	)
	seen := map[string]bool{}
	var monitors []map[string]any
	for _, path := range matches {
		raw, err := os.ReadFile(path)
		if err != nil || len(raw) < 128 {
			continue
		}
		monitor := BuildMonitor(raw)
		if monitor == nil {
			continue
		}
		key, _ := monitor["SERIAL"].(string)
		if key == "" {
			key, _ = monitor["BASE64"].(string)
		}
		if seen[key] {
			continue
		}
		seen[key] = true
		monitors = append(monitors, monitor)
	}
	return monitors
}

// filepathGlob globs several patterns and concatenates the matches.
func filepathGlob(patterns ...string) ([]string, error) {
	var all []string
	for _, p := range patterns {
		m, err := filepath.Glob(p)
		if err != nil {
			return nil, err
		}
		all = append(all, m...)
	}
	return all, nil
}

// runCommand runs a tool found on PATH and returns its stdout, or "" if it is
// unavailable or fails.
func runCommand(name string, args ...string) string {
	path, err := exec.LookPath(name)
	if err != nil {
		return ""
	}
	out, err := exec.Command(path, args...).Output()
	if err != nil {
		return ""
	}
	return string(out)
}

func osEnviron() []string { return os.Environ() }

// collectProcesses walks /proc and builds the PROCESSES entries, resolving uids
// against /etc/passwd.
func collectProcesses() []map[string]any {
	uidToUser := passwdUIDMap()
	btime := procBootTime()
	entries, err := os.ReadDir("/proc")
	if err != nil {
		return nil
	}
	var procs []map[string]any
	for _, e := range entries {
		pid := e.Name()
		if !isAllDigits(pid) {
			continue
		}
		statusFile, err := os.Open("/proc/" + pid + "/status")
		if err != nil {
			continue
		}
		st := ParseProcStatus(statusFile)
		statusFile.Close()
		cmdline, _ := os.ReadFile("/proc/" + pid + "/cmdline")
		entry := processEntry(pid, st, string(cmdline), uidToUser)

		if stat, err := os.ReadFile("/proc/" + pid + "/stat"); err == nil {
			if started := computeStarted(btime, procStarttimeTicks(string(stat))); started != "" {
				entry["STARTED"] = started
			}
		}
		procs = append(procs, entry)
	}
	return procs
}

// procBootTime reads the boot epoch (btime) from /proc/stat.
func procBootTime() int64 {
	data, err := os.ReadFile("/proc/stat")
	if err != nil {
		return 0
	}
	for _, line := range strings.Split(string(data), "\n") {
		if v, ok := strings.CutPrefix(line, "btime "); ok {
			n, _ := strconv.ParseInt(strings.TrimSpace(v), 10, 64)
			return n
		}
	}
	return 0
}

// passwdUIDMap returns a uid -> login map from /etc/passwd.
func passwdUIDMap() map[string]string {
	m := map[string]string{}
	f, err := os.Open("/etc/passwd")
	if err != nil {
		return m
	}
	defer f.Close()
	scanLines(f, func(line string) {
		if line == "" || line[0] == '#' {
			return
		}
		fields := strings.Split(line, ":")
		if len(fields) >= 3 {
			m[fields[2]] = fields[0]
		}
	})
	return m
}

func isAllDigits(s string) bool {
	if s == "" {
		return false
	}
	for _, c := range s {
		if c < '0' || c > '9' {
			return false
		}
	}
	return true
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
