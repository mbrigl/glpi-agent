// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"encoding/base64"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/content"
	"github.com/glpi-project/glpi-agent/go/internal/inventory"
)

// run returns a remote command's stdout (empty on error).
func run(sys remoteSystem, command string) string {
	out, err := sys.Run(command)
	if err != nil {
		return ""
	}
	return out
}

// remoteExists reports whether a path exists on the remote host.
func remoteExists(sys remoteSystem, path string) bool {
	return strings.TrimSpace(run(sys, "test -e "+shellQuote(path)+" && echo yes")) == "yes"
}

// collectRemoteFirewall builds FIREWALL from ufw / firewalld, mirroring
// Linux/Firewall (collect_linux collectFirewall).
func collectRemoteFirewall(sys remoteSystem, inv *content.Inventory) {
	var fw []map[string]any
	if sys.CanRun("ufw") {
		status := "off"
		if strings.Contains(run(sys, "ufw status"), "Status: active") {
			status = "on"
		}
		fw = append(fw, map[string]any{"DESCRIPTION": "ufw", "STATUS": status})
	}
	if sys.CanRun("systemctl") {
		if strings.TrimSpace(run(sys, "systemctl is-active firewalld")) == "active" {
			fw = append(fw, map[string]any{"DESCRIPTION": "firewalld", "STATUS": "on"})
		}
	}
	setIfAny(inv, "FIREWALL", fw)
}

// collectRemoteAntivirus runs the Linux AV detectors over SSH, mirroring
// collect_linux collectAntivirus (the per-product exported parsers).
func collectRemoteAntivirus(sys remoteSystem, inv *content.Inventory) {
	var av []map[string]any
	add := func(e map[string]any) {
		if e != nil {
			av = append(av, e)
		}
	}

	if sys.CanRun("mdatp") {
		add(inventory.ParseDefenderHealth([]byte(run(sys, "mdatp health --output json"))))
	}
	if remoteExists(sys, "/opt/CrowdStrike/falconctl") {
		add(inventory.ParseCrowdStrikeVersion(run(sys, "/opt/CrowdStrike/falconctl -g --version")))
	}
	if bdui := "/opt/bitdefender-security-tools/bin/bduitool"; remoteExists(sys, bdui) {
		add(inventory.ParseBitdefender(run(sys, bdui+" get ps")))
	}
	if cytool := "/opt/traps/bin/cytool"; remoteExists(sys, cytool) {
		add(inventory.ParseCortex(run(sys, cytool+" info"), run(sys, cytool+" info query")))
	}
	if sys.CanRun("drweb-ctl") {
		add(inventory.ParseDrWeb(
			run(sys, "drweb-ctl --version"),
			run(sys, "systemctl is-active drweb-configd.service"),
			run(sys, "drweb-ctl baseinfo"),
		))
	}
	if upd, lic := "/opt/eset/eea/bin/upd", "/opt/eset/eea/sbin/lic"; remoteExists(sys, upd) && remoteExists(sys, lic) {
		add(inventory.ParseEEA(
			run(sys, upd+" -version"),
			run(sys, "systemctl is-active eea.service"),
			run(sys, lic+" --status"),
			run(sys, upd+" --list-modules"),
		))
	}
	if sys.CanRun("kesl-control") {
		add(inventory.ParseKESL(
			run(sys, "systemctl is-active kesl.service"),
			run(sys, "kesl-control --app-info"),
		))
	}
	if sc := "/opt/sentinelone/bin/sentinelctl"; remoteExists(sys, sc) {
		add(inventory.ParseSentinelOne(run(sys, sc+" version") + "\n" +
			run(sys, sc+" engines status") + "\n" +
			run(sys, sc+" control status") + "\n" +
			run(sys, sc+" management status")))
	}
	setIfAny(inv, "ANTIVIRUS", av)
}

// collectRemoteMonitors builds MONITORS from the remote DRM EDID blocks, reading
// them base64-encoded so the binary survives the SSH text channel.
func collectRemoteMonitors(sys remoteSystem, inv *content.Inventory) {
	fs := remoteFS{sys: sys}
	var matches []string
	for _, pattern := range []string{"/sys/devices/*/*/drm/*/edid", "/sys/devices/*/*/*/drm/*/edid"} {
		m, _ := fs.Glob(pattern)
		matches = append(matches, m...)
	}

	seen := map[string]bool{}
	var monitors []map[string]any
	for _, path := range matches {
		raw, err := base64.StdEncoding.DecodeString(strings.TrimSpace(run(sys, "base64 -w0 "+shellQuote(path)+" 2>/dev/null")))
		if err != nil || len(raw) < 128 {
			continue
		}
		monitor := inventory.BuildMonitor(raw)
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
	setIfAny(inv, "MONITORS", monitors)
}

// collectRemoteNetworks builds NETWORKS from the remote /sys/class/net tree plus
// `ip` for the addresses, mirroring linuxInterfaces() + BuildNetworks.
func collectRemoteNetworks(sys remoteSystem, inv *content.Inventory) {
	dirs, _ := remoteFS{sys: sys}.Glob("/sys/class/net/*")
	var ifaces []inventory.NetIface
	for _, dir := range dirs {
		name := filepath.Base(dir)
		ni := inventory.NetIface{
			Name:    name,
			MAC:     firstLineOf(readSys(sys, dir+"/address")),
			Up:      ifaceUp(readSys(sys, dir+"/flags")),
			Virtual: remoteExists(sys, "/sys/devices/virtual/net/"+name),
			Driver:  ueventDriver(readSys(sys, dir+"/device/uevent")),
			Addrs:   parseIPAddrs(run(sys, "ip -o -4 addr show dev "+name)),
		}
		if sp := strings.TrimSpace(firstLineOf(readSys(sys, dir+"/speed"))); sp != "" {
			if n, err := strconv.Atoi(sp); err == nil && n > 0 {
				ni.Speed = n
			}
		}
		ifaces = append(ifaces, ni)
	}
	if nets := inventory.BuildNetworks(ifaces); len(nets) > 0 {
		inv.Content["NETWORKS"] = nets
	}
}

// readSys reads a remote sysfs attribute (empty on error).
func readSys(sys remoteSystem, path string) string {
	out, err := sys.ReadFile(path)
	if err != nil {
		return ""
	}
	return out
}

// ifaceUp parses the hex /sys/class/net/<n>/flags and tests IFF_UP (0x1).
func ifaceUp(flags string) bool {
	flags = strings.TrimSpace(flags)
	flags = strings.TrimPrefix(flags, "0x")
	n, err := strconv.ParseUint(flags, 16, 32)
	return err == nil && n&0x1 != 0
}

// ueventDriver extracts DRIVER= from a device uevent file's content
// (Linux/Networks.pm _getUevent).
func ueventDriver(uevent string) string {
	for _, line := range strings.Split(uevent, "\n") {
		if v, ok := strings.CutPrefix(line, "DRIVER="); ok {
			return strings.TrimSpace(v)
		}
	}
	return ""
}

// parseIPAddrs parses `ip -o -4 addr` lines into IPv4 addr/mask pairs.
func parseIPAddrs(out string) []inventory.NetAddr {
	var addrs []inventory.NetAddr
	for _, line := range strings.Split(out, "\n") {
		fields := strings.Fields(line)
		for i, f := range fields {
			if f == "inet" && i+1 < len(fields) {
				ip, mask := cidrToIPMask(fields[i+1])
				if ip != "" {
					addrs = append(addrs, inventory.NetAddr{IP: ip, Mask: mask})
				}
			}
		}
	}
	return addrs
}

// cidrToIPMask splits "10.0.0.5/24" into the address and dotted netmask.
func cidrToIPMask(cidr string) (ip, mask string) {
	slash := strings.IndexByte(cidr, '/')
	if slash < 0 {
		return cidr, ""
	}
	ip = cidr[:slash]
	prefix, err := strconv.Atoi(cidr[slash+1:])
	if err != nil || prefix < 0 || prefix > 32 {
		return ip, ""
	}
	var m uint32 = 0xffffffff << (32 - uint(prefix))
	if prefix == 0 {
		m = 0
	}
	return ip, strconv.Itoa(int(m>>24&0xff)) + "." + strconv.Itoa(int(m>>16&0xff)) + "." +
		strconv.Itoa(int(m>>8&0xff)) + "." + strconv.Itoa(int(m&0xff))
}
