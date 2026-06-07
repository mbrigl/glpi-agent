// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"io"
	"regexp"
	"strings"
)

// PCIDevice is one device parsed from `lspci -v -nn`, mirroring the hashref
// GLPI::Agent::Tools::Generic::getPCIDevices builds.
type PCIDevice struct {
	PCISlot        string
	Name           string
	PCIClass       string
	Manufacturer   string
	PCIID          string // vendor:device
	Rev            string
	Driver         string
	PCISubsystemID string
	MemoryMB       int
}

var (
	pciHeaderRE = regexp.MustCompile(`(?i)^(\S+)\s+(.+?)\s+\[([0-9a-f]+)\]:\s+(.+)\s+\[([0-9a-f]{4}:[0-9a-f]{4})\](?:\s+\(rev\s+([0-9a-f]+)\))?\s*$`)
	pciDriverRE = regexp.MustCompile(`^\s+Kernel driver in use:\s+(\w+)`)
	pciSubsysRE = regexp.MustCompile(`(?i)^\s+Subsystem:.*\[?([0-9a-f]{4}:[0-9a-f]{4})\]?`)
	pciMemRE    = regexp.MustCompile(`(?i)^\s+Memory.*\sprefetchable.*\[size=([^\]]+)\]`)
)

// ParseLspci parses `lspci -v -nn` output into PCI devices.
func ParseLspci(r io.Reader) []PCIDevice {
	var devices []PCIDevice
	var cur *PCIDevice

	flush := func() {
		if cur != nil {
			devices = append(devices, *cur)
		}
		cur = nil
	}

	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		line := scanner.Text()
		if m := pciHeaderRE.FindStringSubmatch(line); m != nil {
			flush()
			cur = &PCIDevice{
				PCISlot:      m[1],
				Name:         m[2],
				PCIClass:     m[3],
				Manufacturer: m[4],
				PCIID:        m[5],
				Rev:          m[6],
			}
			continue
		}
		if cur == nil {
			continue
		}
		if strings.TrimSpace(line) == "" {
			flush()
			continue
		}
		if m := pciDriverRE.FindStringSubmatch(line); m != nil {
			cur.Driver = m[1]
		} else if m := pciSubsysRE.FindStringSubmatch(line); m != nil {
			cur.PCISubsystemID = m[1]
		} else if m := pciMemRE.FindStringSubmatch(line); m != nil {
			cur.MemoryMB += canonicalSizeMB(strings.TrimSpace(m[1]) + "B")
		}
	}
	flush()
	return devices
}

// BuildControllers assembles CONTROLLERS from the PCI devices, mirroring
// Generic/PCI/Controllers.pm.
func BuildControllers(devices []PCIDevice) []map[string]any {
	var controllers []map[string]any
	for _, d := range devices {
		if d.PCIID == "" {
			continue
		}
		c := map[string]any{
			"NAME":     d.Name,
			"PCICLASS": d.PCIClass,
			"PCISLOT":  d.PCISlot,
		}
		setIf(c, "MANUFACTURER", d.Manufacturer)
		setIf(c, "REV", d.Rev)
		setIf(c, "DRIVER", d.Driver)
		setIf(c, "PCISUBSYSTEMID", d.PCISubsystemID)
		if vendor, device, ok := strings.Cut(d.PCIID, ":"); ok {
			c["VENDORID"] = vendor
			c["PRODUCTID"] = device
		}
		controllers = append(controllers, c)
	}
	return controllers
}

var (
	videoNameRE = regexp.MustCompile(`(?i)graphics|vga|video|display|3D controller`)
	chipsetRE   = regexp.MustCompile(`^(.*)\s+\[(.*)\]$`)
)

// BuildVideos assembles VIDEOS from the VGA/display PCI devices, mirroring
// Generic/PCI/Videos.pm (the pci.ids vendor-database refinement and the X11
// resolution probe are follow-on).
func BuildVideos(devices []PCIDevice) []map[string]any {
	var videos []map[string]any
	for _, d := range devices {
		if !videoNameRE.MatchString(d.Name) {
			continue
		}
		chipset, name := d.Name, d.Manufacturer
		if m := chipsetRE.FindStringSubmatch(d.Manufacturer); m != nil {
			name, chipset = m[1], m[2]
		}
		video := map[string]any{
			"CHIPSET": chipset,
			"NAME":    name,
		}
		setIf(video, "PCIID", d.PCIID)
		setIf(video, "PCISLOT", d.PCISlot)
		if d.MemoryMB > 0 {
			video["MEMORY"] = d.MemoryMB
		}
		videos = append(videos, video)
	}
	return videos
}

// BuildSounds assembles SOUNDS from the audio PCI devices, mirroring
// Generic/PCI/Sounds.pm.
func BuildSounds(devices []PCIDevice) []map[string]any {
	var sounds []map[string]any
	for _, d := range devices {
		if !strings.Contains(strings.ToLower(d.Name), "audio") {
			continue
		}
		sound := map[string]any{"NAME": d.Name}
		setIf(sound, "MANUFACTURER", d.Manufacturer)
		if d.Rev != "" {
			sound["DESCRIPTION"] = "rev " + d.Rev
		}
		sounds = append(sounds, sound)
	}
	return sounds
}
