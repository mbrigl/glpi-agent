// SPDX-License-Identifier: GPL-2.0-only

package vsphere

import (
	"fmt"
	"strings"
	"time"

	"github.com/vmware/govmomi/vim25/mo"
	"github.com/vmware/govmomi/vim25/types"

	"github.com/glpi-project/glpi-agent/go/internal/content"
)

// buildInventory assembles one host inventory, mirroring
// GLPI::Agent::Task::ESX::createInventory: BIOS, HARDWARE, OPERATINGSYSTEM,
// CPUS, CONTROLLERS, NETWORKS, STORAGES, DRIVES and VIRTUALMACHINES.
func buildInventory(host *mo.HostSystem, vms []mo.VirtualMachine, itemtype, tag string) *content.Inventory {
	if itemtype == "" {
		itemtype = "Computer"
	}
	inv := content.New(content.DeviceID(hostName(host), time.Now()))
	inv.ItemType = itemtype
	if tag != "" {
		inv.Content["ACCOUNTINFO"] = map[string]any{"KEYNAME": "TAG", "KEYVALUE": tag}
	}

	if bios := biosInfo(host); bios != nil {
		inv.Content["BIOS"] = bios
	}
	inv.Content["HARDWARE"] = hardwareInfo(host)
	if os := operatingSystemInfo(host); os != nil {
		inv.Content["OPERATINGSYSTEM"] = os
	}
	if cpus := cpus(host); len(cpus) > 0 {
		inv.Content["CPUS"] = cpus
	}
	if ctrls := controllers(host); len(ctrls) > 0 {
		inv.Content["CONTROLLERS"] = ctrls
	}
	if nets := networks(host); len(nets) > 0 {
		inv.Content["NETWORKS"] = nets
	}
	if st := storages(host); len(st) > 0 {
		inv.Content["STORAGES"] = st
	}
	if dr := drives(host); len(dr) > 0 {
		inv.Content["DRIVES"] = dr
	}
	if machines := virtualMachines(vms); len(machines) > 0 {
		inv.Content["VIRTUALMACHINES"] = machines
	}
	return inv
}

// hostName mirrors getHostname: the DNS hostName, falling back to the managed
// object name.
func hostName(host *mo.HostSystem) string {
	if dns := dnsConfig(host); dns != nil && dns.HostName != "" {
		return dns.HostName
	}
	if host.Name != "" {
		return host.Name
	}
	return "esx-host"
}

func dnsConfig(host *mo.HostSystem) *types.HostDnsConfig {
	if host.Config == nil || host.Config.Network == nil || host.Config.Network.DnsConfig == nil {
		return nil
	}
	return host.Config.Network.DnsConfig.GetHostDnsConfig()
}

// biosInfo mirrors getBiosInfo. The serial-number disambiguation over
// otherIdentifyingInfo (ServiceTag/AssetTag/Enclosure/SerialNumber) follows the
// Perl logic.
func biosInfo(host *mo.HostSystem) map[string]any {
	if host.Hardware == nil || host.Hardware.BiosInfo == nil {
		return nil
	}
	sys := host.Hardware.SystemInfo
	bios := map[string]any{
		"BVERSION":      host.Hardware.BiosInfo.BiosVersion,
		"SMODEL":        sys.Model,
		"SMANUFACTURER": sys.Vendor,
	}
	if d := host.Hardware.BiosInfo.ReleaseDate; d != nil {
		bios["BDATE"] = d.Format("2006-01-02")
	}
	for _, info := range sys.OtherIdentifyingInfo {
		val := strings.TrimSpace(info.IdentifierValue)
		if isInvalidBiosValue(val) {
			continue
		}
		switch info.IdentifierType.GetElementDescription().Key {
		case "ServiceTag":
			if ssn, ok := bios["SSN"].(string); ok && ssn != "" {
				bios["MSN"] = ssn
			}
			bios["SSN"] = val
		case "AssetTag":
			bios["ASSETTAG"] = val
		case "EnclosureSerialNumberTag":
			bios["MSN"] = val
		case "SerialNumberTag":
			bios["SSN"] = val
		}
	}
	return bios
}

// isInvalidBiosValue mirrors GLPI::Agent::Tools::isInvalidBiosValue for the
// values ESX reports as placeholders.
func isInvalidBiosValue(v string) bool {
	switch strings.ToLower(strings.TrimSpace(v)) {
	case "", "n/a", "na", "none", "unknown", "default string", "not specified", "not available":
		return true
	}
	return false
}

// hardwareInfo mirrors getHardwareInfo.
func hardwareInfo(host *mo.HostSystem) map[string]any {
	hw := map[string]any{}
	if dns := dnsConfig(host); dns != nil {
		hw["NAME"] = dns.HostName
		hw["WORKGROUP"] = dns.DomainName
		hw["DNS"] = strings.Join(dns.Address, "/")
	}
	if host.Hardware != nil {
		hw["MEMORY"] = int(host.Hardware.MemorySize / (1024 * 1024))
		hw["UUID"] = firstNonEmpty(host.Summary.Hardware.Uuid, host.Hardware.SystemInfo.Uuid)
	}
	return hw
}

// operatingSystemInfo mirrors getOperatingSystemInfo, including the timezone
// offset formatting.
func operatingSystemInfo(host *mo.HostSystem) map[string]any {
	product := host.Summary.Config.Product
	os := map[string]any{"FQDN": host.Name}
	if product != nil {
		os["NAME"] = product.Name
		os["VERSION"] = product.Version
		os["FULL_NAME"] = product.FullName
	}
	if dns := dnsConfig(host); dns != nil {
		os["DNS_DOMAIN"] = dns.DomainName
	}
	if bt := host.Summary.Runtime.BootTime; bt != nil {
		os["BOOT_TIME"] = bt.Format("2006-01-02 15:04:05")
	}
	if host.Config != nil && host.Config.DateTimeInfo != nil && host.Config.DateTimeInfo.TimeZone.Key != "" {
		tz := host.Config.DateTimeInfo.TimeZone
		offset := tz.GmtOffset / 3600
		sign := "+"
		if offset < 0 {
			sign = "-"
			offset = -offset
		}
		os["TIMEZONE"] = map[string]any{
			"NAME":   tz.Name,
			"OFFSET": fmt.Sprintf("%s%04d", sign, offset*100),
		}
	}
	return os
}

// cpus mirrors getCPUs.
func cpus(host *mo.HostSystem) []map[string]any {
	if host.Hardware == nil {
		return nil
	}
	manufacturer := map[string]string{"amd": "AMD", "intel": "Intel"}
	info := host.Hardware.CpuInfo
	totalCore := int(info.NumCpuCores)
	totalThread := int(info.NumCpuThreads)
	packages := int(info.NumCpuPackages)
	if packages == 0 {
		packages = len(host.Hardware.CpuPkg)
	}

	var out []map[string]any
	for _, pkg := range host.Hardware.CpuPkg {
		cpu := map[string]any{
			"NAME":  pkg.Description,
			"SPEED": int(pkg.Hz / (1000 * 1000)),
		}
		if m, ok := manufacturer[strings.ToLower(pkg.Vendor)]; ok {
			cpu["MANUFACTURER"] = m
		} else {
			cpu["MANUFACTURER"] = pkg.Vendor
		}
		if packages > 0 {
			cpu["CORE"] = totalCore / packages
		}
		if totalCore > 0 {
			cpu["THREAD"] = totalThread / totalCore
		}
		out = append(out, cpu)
	}
	return out
}

// controllers mirrors getControllers (PCI devices), including the 4-hex-digit
// class/vendor/product ids and optional subsystem id.
func controllers(host *mo.HostSystem) []map[string]any {
	if host.Hardware == nil {
		return nil
	}
	var out []map[string]any
	for _, d := range host.Hardware.PciDevice {
		ctrl := map[string]any{
			"NAME":         d.DeviceName,
			"MANUFACTURER": d.VendorName,
			"PCICLASS":     hex4(d.ClassId),
			"VENDORID":     hex4(d.VendorId),
			"PRODUCTID":    hex4(d.DeviceId),
			"PCISLOT":      d.Id,
		}
		if d.SubVendorId != 0 || d.SubDeviceId != 0 {
			ctrl["PCISUBSYSTEMID"] = hex4(d.SubVendorId) + ":" + hex4(d.SubDeviceId)
		}
		out = append(out, ctrl)
	}
	return out
}

// networks mirrors getNetworks: physical, virtual and console NICs, de-duplicated
// by device name.
func networks(host *mo.HostSystem) []map[string]any {
	if host.Config == nil || host.Config.Network == nil {
		return nil
	}
	net := host.Config.Network
	seen := map[string]bool{}
	var out []map[string]any

	for _, p := range net.Pnic {
		if seen[p.Device] {
			continue
		}
		seen[p.Device] = true
		out = append(out, nicFromPnic(p))
	}
	for _, v := range net.Vnic {
		if seen[v.Device] {
			continue
		}
		seen[v.Device] = true
		out = append(out, nicFromVnic(v))
	}
	for _, v := range net.ConsoleVnic {
		if seen[v.Device] {
			continue
		}
		seen[v.Device] = true
		out = append(out, nicFromVnic(v))
	}
	return out
}

func nicFromPnic(p types.PhysicalNic) map[string]any {
	nic := map[string]any{"VIRTUALDEV": 0}
	setIfNotEmpty(nic, "DESCRIPTION", p.Device)
	setIfNotEmpty(nic, "DRIVER", p.Driver)
	setIfNotEmpty(nic, "PCISLOT", p.Pci)
	setIfNotEmpty(nic, "MACADDR", p.Mac)
	if p.LinkSpeed != nil && p.LinkSpeed.SpeedMb != 0 {
		nic["SPEED"] = int(p.LinkSpeed.SpeedMb)
	}
	nic["STATUS"] = "Down"
	return nic
}

func nicFromVnic(v types.HostVirtualNic) map[string]any {
	nic := map[string]any{"VIRTUALDEV": 1}
	setIfNotEmpty(nic, "DESCRIPTION", v.Device)
	setIfNotEmpty(nic, "MACADDR", v.Spec.Mac)
	status := "Down"
	if v.Spec.Ip != nil {
		setIfNotEmpty(nic, "IPADDRESS", v.Spec.Ip.IpAddress)
		setIfNotEmpty(nic, "IPMASK", v.Spec.Ip.SubnetMask)
		if v.Spec.Ip.IpAddress != "" {
			status = "Up"
		}
	}
	if v.Spec.Mtu != 0 {
		nic["MTU"] = int(v.Spec.Mtu)
	}
	nic["STATUS"] = status
	return nic
}

// storages mirrors getStorages (SCSI LUNs).
func storages(host *mo.HostSystem) []map[string]any {
	if host.Config == nil || host.Config.StorageDevice == nil {
		return nil
	}
	var out []map[string]any
	for _, base := range host.Config.StorageDevice.ScsiLun {
		lun := base.GetScsiLun()
		st := map[string]any{
			"DESCRIPTION": lun.DisplayName,
			"NAME":        lun.DeviceName,
			"TYPE":        lun.DeviceType,
			"FIRMWARE":    lun.Revision,
			"MODEL":       strings.TrimSpace(lun.Model),
		}
		vendor := strings.TrimSpace(lun.Vendor)
		if vendor != "" && strings.TrimSpace(vendor) != "ATA" {
			st["MANUFACTURER"] = vendor
		} else {
			st["MANUFACTURER"] = strings.TrimSpace(lun.Model)
		}
		var serial string
		for _, alt := range lun.AlternateName {
			if alt.Namespace == "SERIALNUM" {
				serial += string(alt.Data)
			}
		}
		if serial != "" {
			st["SERIAL"] = serial
		}
		if disk, ok := base.(*types.HostScsiDisk); ok {
			size := int(disk.Capacity.BlockSize) * int(disk.Capacity.Block) / 1024 / 1024
			if size > 0 {
				st["DISKSIZE"] = size
			}
		}
		out = append(out, st)
	}
	return out
}

// drives mirrors getDrives (mounted file-system volumes).
func drives(host *mo.HostSystem) []map[string]any {
	if host.Config == nil || host.Config.FileSystemVolume == nil {
		return nil
	}
	var out []map[string]any
	for _, mi := range host.Config.FileSystemVolume.MountInfo {
		vol := mi.Volume.GetHostFileSystemVolume()
		if vol == nil {
			continue
		}
		drive := map[string]any{
			"SERIAL":     "",
			"TYPE":       mi.MountInfo.Path,
			"LABEL":      vol.Name,
			"FILESYSTEM": strings.ToLower(vol.Type),
			"TOTAL":      int(vol.Capacity / (1000 * 1000)),
		}
		out = append(out, drive)
	}
	return out
}

func hex4(v int16) string { return fmt.Sprintf("%04x", uint16(v)) }

func setIfNotEmpty(m map[string]any, key, val string) {
	if val != "" {
		m[key] = val
	}
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}
