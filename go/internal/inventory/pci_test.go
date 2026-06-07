// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

const lspciSample = `00:02.0 VGA compatible controller [0300]: Intel Corporation Raptor Lake-S GT1 [UHD Graphics 770] [8086:a780] (rev 04)
	Subsystem: Dell Device [1028:0ac7]
	Kernel driver in use: i915
	Memory at 4000000000 (64-bit, prefetchable) [size=256M]

00:1f.3 Audio device [0403]: Intel Corporation Raptor Lake High Definition Audio [8086:7a50] (rev 11)
	Subsystem: Dell Device [1028:0ac7]
	Kernel driver in use: snd_hda_intel

02:00.0 Ethernet controller [0200]: Realtek RTL8111 [10ec:8168] (rev 15)
	Kernel driver in use: r8169
`

func TestParseLspciAndBuilders(t *testing.T) {
	devices := ParseLspci(strings.NewReader(lspciSample))
	if len(devices) != 3 {
		t.Fatalf("parsed %d devices, want 3", len(devices))
	}

	// Header parse: the VGA device.
	vga := devices[0]
	if vga.PCISlot != "00:02.0" || vga.PCIClass != "0300" || vga.PCIID != "8086:a780" || vga.Rev != "04" {
		t.Errorf("vga header = %+v", vga)
	}
	if vga.Driver != "i915" || vga.PCISubsystemID != "1028:0ac7" || vga.MemoryMB != 256 {
		t.Errorf("vga details = %+v", vga)
	}

	// CONTROLLERS: all three, with vendor/product split.
	controllers := BuildControllers(devices)
	if len(controllers) != 3 {
		t.Fatalf("controllers = %d, want 3", len(controllers))
	}
	if controllers[0]["VENDORID"] != "8086" || controllers[0]["PRODUCTID"] != "a780" {
		t.Errorf("controller id split wrong: %v", controllers[0])
	}
	if controllers[0]["DRIVER"] != "i915" {
		t.Errorf("controller driver = %v", controllers[0]["DRIVER"])
	}

	// VIDEOS: only the VGA device, CHIPSET parsed from the bracketed name.
	videos := BuildVideos(devices)
	if len(videos) != 1 {
		t.Fatalf("videos = %d, want 1", len(videos))
	}
	if videos[0]["CHIPSET"] != "UHD Graphics 770" {
		t.Errorf("CHIPSET = %v, want 'UHD Graphics 770'", videos[0]["CHIPSET"])
	}
	if videos[0]["NAME"] != "Intel Corporation Raptor Lake-S GT1" {
		t.Errorf("NAME = %v", videos[0]["NAME"])
	}
	if videos[0]["MEMORY"] != 256 {
		t.Errorf("MEMORY = %v, want 256", videos[0]["MEMORY"])
	}

	// SOUNDS: only the audio device.
	sounds := BuildSounds(devices)
	if len(sounds) != 1 {
		t.Fatalf("sounds = %d, want 1", len(sounds))
	}
	if !strings.Contains(sounds[0]["NAME"].(string), "Audio") || sounds[0]["DESCRIPTION"] != "rev 11" {
		t.Errorf("sound = %v", sounds[0])
	}
}
