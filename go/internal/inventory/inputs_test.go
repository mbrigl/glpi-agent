// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestParseInputDevices(t *testing.T) {
	const devices = `I: Bus=0011 Vendor=0001 Product=0001 Version=ab41
N: Name="AT Translated Set 2 keyboard"
P: Phys=isa0060/serio0/input0
H: Handlers=kbd event0 leds

I: Bus=0019 Vendor=0000 Product=0001 Version=0000
N: Name="Power Button"
P: Phys=PNP0C0C/button/input0
H: Handlers=kbd event1

I: Bus=0003 Vendor=046d Product=c52b Version=0111
N: Name="Logitech USB Mouse"
P: Phys=usb-0000:00:14.0-1/input0
H: Handlers=mouse0 event2

`
	inputs := ParseInputDevices(strings.NewReader(devices))
	if len(inputs) != 2 {
		t.Fatalf("got %d inputs, want 2 (the button has no input phys)", len(inputs))
	}
	if inputs[0]["DESCRIPTION"] != "AT Translated Set 2 keyboard" || inputs[0]["TYPE"] != "Keyboard" {
		t.Errorf("keyboard = %v", inputs[0])
	}
	if inputs[1]["CAPTION"] != "Logitech USB Mouse" || inputs[1]["TYPE"] != "Pointing" {
		t.Errorf("mouse = %v", inputs[1])
	}
}
