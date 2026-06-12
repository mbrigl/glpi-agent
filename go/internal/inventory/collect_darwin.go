// SPDX-License-Identifier: GPL-2.0-only

//go:build darwin

package inventory

import (
	"os"
	"os/exec"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// macUptimeSeconds returns the seconds since boot from `sysctl -n kern.boottime`,
// mirroring Tools/MacOS.pm getBootTime + Uptime.pm.
func macUptimeSeconds() int {
	out := commandOutput("sysctl", "-n", "kern.boottime")
	m := regexp.MustCompile(`sec = (\d+)`).FindStringSubmatch(out)
	if m == nil {
		return 0
	}
	boot, err := strconv.ParseInt(m[1], 10, 64)
	if err != nil {
		return 0
	}
	up := time.Now().Unix() - boot
	if up < 0 {
		return 0
	}
	return int(up)
}

// Collect gathers the local macOS inventory via system_profiler, sysctl, ioreg
// and uname, mirroring the upstream Task/Inventory/MacOS/* modules. The parsing
// is pure (macos*.go) and unit-tested on Linux against vendored fixtures.
func Collect() Sections {
	s := Sections{}
	s.mergeHardware(map[string]any{"NAME": hostname()})

	software := systemProfiler("SPSoftwareDataType")
	hardware := systemProfiler("SPHardwareDataType")

	// operatingsystem (system_profiler + uname + boottime).
	os := buildMacOS(software)
	setIf(os, "KERNEL_VERSION", strings.TrimSpace(uname("-r")))
	setIf(os, "ARCH", strings.TrimSpace(uname("-m")))
	s["OPERATINGSYSTEM"] = os

	// hardware (system_profiler; UUID from ioreg as a fallback). The Computer
	// Name (Hostname.pm) overrides the kernel hostname; DESCRIPTION carries
	// "<arch>/<uptime>" (Uptime.pm).
	hw := buildMacHardware(software, hardware)
	if _, ok := hw["UUID"]; !ok {
		if uuid := ioregUUID(); uuid != "" {
			hw["UUID"] = uuid
		}
	}
	if name := macHostname(software); name != "" {
		hw["NAME"] = name
	}
	if arch := strings.TrimSpace(uname("-m")); arch != "" {
		if up := macUptimeSeconds(); up > 0 {
			hw["DESCRIPTION"] = arch + "/" + strconv.Itoa(up)
		}
	}
	s.mergeHardware(hw)

	// cpus (SPHardwareDataType "Hardware Overview" + sysctl machdep.cpu).
	overview := spNode(hardware, "Hardware", "Hardware Overview")
	sysctl := parseSysctl(commandOutput("sysctl", "-a", "machdep.cpu"))
	if cpus := buildMacCPUs(overview, sysctl); len(cpus) > 0 {
		s["CPUS"] = cpus
	}

	// memories (SPMemoryDataType) + hardware MEMORY (from SPHardwareDataType).
	memory := systemProfiler("SPMemoryDataType")
	if mems := buildMacMemories(memory); len(mems) > 0 {
		s["MEMORIES"] = mems
	}
	if total := macTotalMemoryMB(hardware); total > 0 {
		s.mergeHardware(map[string]any{"MEMORY": total})
	}

	// bios (SPHardwareDataType + ioreg IOPlatformExpertDevice).
	s["BIOS"] = buildMacBios(overview, ioregDevice())

	// videos (SPDisplaysDataType).
	if v := buildMacVideos(systemProfiler("SPDisplaysDataType")); len(v) > 0 {
		s["VIDEOS"] = v
	}

	// power: AC charger (POWERSUPPLIES) + battery (BATTERIES) from SPPowerDataType.
	power := systemProfiler("SPPowerDataType")
	if psu := buildMacCharger(power); psu != nil {
		s["POWERSUPPLIES"] = []map[string]any{psu}
	}
	if battery := buildMacBattery(power); battery != nil {
		s["BATTERIES"] = []map[string]any{battery}
	}

	// sounds (SPAudioDataType "Audio (Built In)").
	if snd := buildMacSounds(systemProfiler("SPAudioDataType")); len(snd) > 0 {
		s["SOUNDS"] = snd
	}

	// networks (ifconfig joined with networksetup hardware ports).
	netsetup := parseMacNetworkSetup(commandOutput("networksetup", "-listallhardwareports"))
	if n := buildMacNetworks(commandOutput("/sbin/ifconfig", "-a"), netsetup); len(n) > 0 {
		s["NETWORKS"] = n
	}

	// storages (every storage-bearing system_profiler datatype, plist XML).
	var storages []map[string]any
	storageType := func(dt string, fn func(any) []map[string]any) {
		if root, err := parsePlist([]byte(commandOutput("/usr/sbin/system_profiler", "-xml", dt))); err == nil {
			storages = append(storages, fn(root)...)
		}
	}
	storageType("SPSerialATADataType", func(r any) []map[string]any { return buildMacATAStorages(r, "SATA", true) })
	storageType("SPNVMeDataType", func(r any) []map[string]any { return buildMacATAStorages(r, "NVME", false) })
	storageType("SPDiscBurningDataType", buildMacDiscBurningStorages)
	storageType("SPCardReaderDataType", buildMacCardReaderStorages)
	storageType("SPUSBDataType", func(r any) []map[string]any { return buildMacUSBStorages(r, "_items", "USB") })
	storageType("SPFireWireDataType", func(r any) []map[string]any { return buildMacUSBStorages(r, "units", "1394") })
	if len(storages) > 0 {
		s["STORAGES"] = storages
	}

	// firewall (application-firewall service + globalstate).
	fwRunning := regexp.MustCompile(`(?m)^\d+\s+\S+\s+com\.apple\.alf$`).MatchString(commandOutput("launchctl", "list"))
	fwState := regexp.MustCompile(`(?m)^(\d)$`).FindStringSubmatch(commandOutput("defaults", "read", "/Library/Preferences/com.apple.alf", "globalstate"))
	state := ""
	if fwState != nil {
		state = fwState[1]
	}
	s["FIREWALL"] = []map[string]any{{"STATUS": macFirewallStatus(fwRunning, state)}}

	// drives (mount types -> df per type, joined with diskutil info + FileVault).
	skipFS := map[string]bool{"fdesc": true, "devfs": true, "procfs": true, "linprocfs": true,
		"linsysfs": true, "tmpfs": true, "fdescfs": true}
	var filesystems []map[string]any
	for _, fsType := range parseMacMountTypes(commandOutput("mount")) {
		if skipFS[fsType] {
			continue
		}
		filesystems = append(filesystems, parseMacDf(commandOutput("df", "-P", "-k", "-t", fsType), fsType)...)
	}
	partitionInfo := map[string]map[string]string{}
	for _, p := range parseMacDiskutilPartitions(commandOutput("diskutil", "list")) {
		partitionInfo[p] = parseMacDiskutilInfo(commandOutput("diskutil", "info", p))
	}
	fileVaultOn := regexp.MustCompile(`(?i)FileVault is On`).MatchString(commandOutput("fdesetup", "status"))
	if drives := buildMacDrives(filesystems, partitionInfo, fileVaultOn); len(drives) > 0 {
		s["DRIVES"] = drives
	}

	// softwares (SPApplicationsDataType, plist XML). The lastModified dates are
	// shifted by the local timezone offset (detectLocalTimeOffset).
	_, offset := time.Now().Zone()
	if root, err := parsePlist([]byte(commandOutput("/usr/sbin/system_profiler", "-xml", "SPApplicationsDataType"))); err == nil {
		if sw := buildMacSoftwares(extractMacSoftwaresFromXML(root, offset)); len(sw) > 0 {
			s["SOFTWARES"] = sw
		}
	}

	// usb devices (ioreg IOUSBDevice), deduplicated by serial.
	usb := buildMacUSB(parseIORegDevices(commandOutput("ioreg", "-c", "IOUSBDevice", "-r", "-l", "-w0", "-d1"), "IOUSBDevice"))
	seen := map[string]bool{}
	var usbDevices []map[string]any
	for _, u := range usb {
		serial, _ := u["SERIAL"].(string)
		if serial != "" {
			if seen[serial] {
				continue
			}
			seen[serial] = true
		}
		usbDevices = append(usbDevices, u)
	}
	if len(usbDevices) > 0 {
		s["USBDEVICES"] = usbDevices
	}

	// antivirus (vendor detectors; each gated on its CLI being installed).
	if av := collectMacAntivirus(); len(av) > 0 {
		s["ANTIVIRUS"] = av
	}

	return s
}

// collectMacAntivirus runs the supported AV vendor detectors, mirroring the
// MacOS/AntiVirus/* modules (each enabled only when its CLI is present).
func collectMacAntivirus() []map[string]any {
	var entries []map[string]any

	if canRunMac("/usr/local/bin/mdatp") {
		if av := buildMacDefender([]byte(commandOutput("/usr/local/bin/mdatp", "health", "--output", "json"))); av != nil {
			entries = append(entries, av)
		}
	}
	if cytool := macFindCommand("/usr/local/bin/cytool"); cytool != "" {
		entries = append(entries, buildMacCortex(
			commandOutput(cytool, "info"),
			commandOutput(cytool, "info", "query"),
			commandOutput(cytool, "runtime", "query"),
		))
	}
	if sentinel := macFindCommand("/usr/local/bin/sentinelctl"); sentinel != "" {
		entries = append(entries, buildMacSentinelOne(
			commandOutput(sentinel, "version"), commandOutput(sentinel, "status")))
	}
	if falconctl := macFindCommand("/Applications/Falcon.app/Contents/Resources/falconctl"); falconctl != "" {
		entries = append(entries, buildMacCrowdStrike(commandOutput(falconctl, "stats", "agent_info")))
	}
	if canRunMac("/usr/local/bin/wsav") {
		running := commandOutput("pgrep", "-x", "wsavd") != ""
		if av := buildMacWithSecure(commandOutput("/usr/local/bin/wsav", "--version"), running); av != nil {
			entries = append(entries, av)
		}
	}
	return entries
}

// canRunMac reports whether path is an executable file.
func canRunMac(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir() && info.Mode()&0o111 != 0
}

// macFindCommand returns the first existing executable from the candidates.
func macFindCommand(candidates ...string) string {
	for _, c := range candidates {
		if canRunMac(c) {
			return c
		}
	}
	return ""
}

// ioregDevice returns the IOPlatformExpertDevice attributes as a flat map.
func ioregDevice() map[string]any {
	out, err := exec.Command("ioreg", "-rd1", "-c", "IOPlatformExpertDevice").Output()
	if err != nil {
		return nil
	}
	dev := map[string]any{}
	re := regexp.MustCompile(`"([^"]+)"\s*=\s*(?:<?)"?([^"<>]*)"?`)
	for _, line := range strings.Split(string(out), "\n") {
		if m := re.FindStringSubmatch(line); m != nil {
			dev[m[1]] = strings.TrimSpace(m[2])
		}
	}
	return dev
}

// commandOutput runs a command and returns its stdout (empty on error).
func commandOutput(name string, args ...string) string {
	out, err := exec.Command(name, args...).Output()
	if err != nil {
		return ""
	}
	return string(out)
}

// systemProfiler runs `system_profiler <dataType>` and parses the text output.
func systemProfiler(dataType string) map[string]any {
	out, err := exec.Command("/usr/sbin/system_profiler", dataType).Output()
	if err != nil {
		return map[string]any{}
	}
	return parseSystemProfiler(string(out))
}

// uname runs `uname <flag>` and returns its output.
func uname(flag string) string {
	out, err := exec.Command("uname", flag).Output()
	if err != nil {
		return ""
	}
	return string(out)
}

// ioregUUID reads IOPlatformUUID from the IOPlatformExpertDevice ioreg node.
func ioregUUID() string {
	out, err := exec.Command("ioreg", "-rd1", "-c", "IOPlatformExpertDevice").Output()
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(out), "\n") {
		if strings.Contains(line, "IOPlatformUUID") {
			if i := strings.Index(line, "= "); i >= 0 {
				return strings.Trim(strings.TrimSpace(line[i+2:]), `"`)
			}
		}
	}
	return ""
}
