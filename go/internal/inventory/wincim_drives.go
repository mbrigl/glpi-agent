// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strconv"
	"strings"
)

var (
	winLogicalDiskProperties = []string{
		"InstallDate", "Description", "FreeSpace", "FileSystem", "VolumeName",
		"Caption", "VolumeSerialNumber", "DeviceID", "Size", "DriveType", "ProviderName",
	}
	winDiskDriveProperties = []string{
		"Manufacturer", "Model", "Caption", "Description", "Name", "MediaType",
		"InterfaceType", "FirmwareRevision", "Size", "SerialNumber", "DeviceID",
	}

	// Win32_LogicalDisk.DriveType (Win32/Drives.pm @type).
	winDriveTypeVal = []string{
		"Unknown", "No Root Directory", "Removable Disk", "Local Disk",
		"Network Drive", "Compact Disc", "RAM Disk",
	}
)

// buildWinDrives maps Win32_LogicalDisk objects to DRIVES entries, mirroring
// Win32/Drives.pm. systemDrive (Win32_OperatingSystem.SystemDrive, e.g. "C:")
// flags the system volume. The Win32_Volume mount-point scan and the network
// ProviderName remap are follow-on.
func buildWinDrives(disks []map[string]any, systemDrive string) []map[string]any {
	systemDrive = strings.ToLower(strings.TrimSpace(systemDrive))
	var drives []map[string]any
	for _, o := range disks {
		d := map[string]any{}
		setIf(d, "CREATEDATE", wmiDateTime(cimString(o, "InstallDate")))
		setIf(d, "DESCRIPTION", cimString(o, "Description"))
		if free := cimBytesToMB(o, "FreeSpace"); free > 0 {
			d["FREE"] = free
		}
		setIf(d, "FILESYSTEM", cimString(o, "FileSystem"))
		setIf(d, "LABEL", cimString(o, "VolumeName"))
		setIf(d, "LETTER", winFirstNonEmpty(cimString(o, "DeviceID"), cimString(o, "Caption")))
		setIf(d, "SERIAL", cimString(o, "VolumeSerialNumber"))
		if total := cimBytesToMB(o, "Size"); total > 0 {
			d["TOTAL"] = total
		}
		setIf(d, "TYPE", enumAt(winDriveTypeVal, cimInt(o, "DriveType")))
		setIf(d, "VOLUMN", cimString(o, "VolumeName"))
		if systemDrive != "" && strings.ToLower(cimString(o, "DeviceID")) == systemDrive {
			d["SYSTEMDRIVE"] = 1
		}
		drives = append(drives, d)
	}
	return drives
}

// buildWinStorages maps Win32_DiskDrive objects to STORAGES entries, mirroring
// the Win32_DiskDrive branch of Win32/Storages.pm. The MSFT_PhysicalDisk class,
// the hdparm refinement and the VBOX serial decode are follow-on.
func buildWinStorages(disks []map[string]any) []map[string]any {
	var storages []map[string]any
	for _, o := range disks {
		st := map[string]any{}
		setIf(st, "MANUFACTURER", cimString(o, "Manufacturer"))
		setIf(st, "MODEL", winFirstNonEmpty(cimString(o, "Model"), cimString(o, "Caption"), cimString(o, "FriendlyName")))
		setIf(st, "DESCRIPTION", winFirstNonEmpty(cimString(o, "Description"), cimString(o, "PhysicalLocation")))
		name := cimString(o, "Name")
		if name == "" {
			name = "PhysicalDisk" + winFirstNonEmpty(cimString(o, "DeviceId"), "0")
		}
		st["NAME"] = name
		setIf(st, "TYPE", cimString(o, "MediaType"))
		setIf(st, "INTERFACE", cimString(o, "InterfaceType"))
		setIf(st, "FIRMWARE", strings.TrimSpace(winFirstNonEmpty(cimString(o, "FirmwareVersion"), cimString(o, "FirmwareRevision"))))
		if sz := cimInt64(o, "Size"); sz > 0 {
			st["DISKSIZE"] = int(sz / 1_000_000) // decimal MB, as upstream
		}
		// Serial: first whitespace-delimited token of the (adapter or disk) serial.
		serial := strings.TrimSpace(winFirstNonEmpty(cimString(o, "AdapterSerialNumber"), cimString(o, "SerialNumber")))
		if serial != "" {
			serial = strings.Fields(serial)[0]
			st["SERIAL"] = serial
		}
		storages = append(storages, st)
	}
	return storages
}

// cimInt64 reads a 64-bit integer CIM property (a JSON number or numeric string).
func cimInt64(obj map[string]any, key string) int64 {
	n, err := strconv.ParseInt(cimString(obj, key), 10, 64)
	if err != nil {
		return 0
	}
	return n
}
