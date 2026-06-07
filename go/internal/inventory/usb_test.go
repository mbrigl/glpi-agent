// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestBuildUSB(t *testing.T) {
	root := t.TempDir()
	dev := "sys/bus/usb/devices/1-1/"
	writeFile(t, root, dev+"idVendor", "046d\n")
	writeFile(t, root, dev+"idProduct", "c52b\n")
	writeFile(t, root, dev+"bDeviceClass", "00\n")
	writeFile(t, root, dev+"bDeviceSubClass", "00\n")
	writeFile(t, root, dev+"product", "Logitech USB Receiver\n")
	writeFile(t, root, dev+"manufacturer", "Logitech\n")
	writeFile(t, root, dev+"serial", "ABCDEF123\n")

	// An interface node (no idVendor) must be ignored.
	writeFile(t, root, "sys/bus/usb/devices/1-1:1.0/bInterfaceClass", "03\n")
	// A device with a too-short serial: serial dropped but device kept.
	short := "sys/bus/usb/devices/2-1/"
	writeFile(t, root, short+"idVendor", "1d6b\n")
	writeFile(t, root, short+"idProduct", "0002\n")
	writeFile(t, root, short+"serial", "x\n")

	usb := BuildUSB(root)
	if len(usb) != 2 {
		t.Fatalf("got %d devices, want 2 (interface ignored)", len(usb))
	}

	byVendor := map[string]map[string]any{}
	for _, d := range usb {
		byVendor[d["VENDORID"].(string)] = d
	}
	logi := byVendor["046d"]
	if logi["PRODUCTID"] != "c52b" || logi["NAME"] != "Logitech USB Receiver" || logi["SERIAL"] != "ABCDEF123" {
		t.Errorf("logitech = %v", logi)
	}
	if _, present := byVendor["1d6b"]["SERIAL"]; present {
		t.Errorf("short serial should be dropped: %v", byVendor["1d6b"]["SERIAL"])
	}
}
