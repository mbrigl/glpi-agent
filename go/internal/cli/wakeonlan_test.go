// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"bytes"
	"testing"
)

// TestMagicPayload checks the Wake-on-LAN payload against _getPayload in
// lib/GLPI/Agent/Task/WakeOnLan.pm: 6 sync bytes (0xFF) then the MAC 16 times.
func TestMagicPayload(t *testing.T) {
	payload, err := magicPayload("01:02:03:04:05:06")
	if err != nil {
		t.Fatal(err)
	}
	if len(payload) != 6+16*6 {
		t.Fatalf("payload length = %d, want %d", len(payload), 6+16*6)
	}
	if !bytes.Equal(payload[:6], []byte{0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF}) {
		t.Errorf("first 6 bytes = % x, want all 0xFF", payload[:6])
	}
	mac := []byte{0x01, 0x02, 0x03, 0x04, 0x05, 0x06}
	for i := 0; i < 16; i++ {
		off := 6 + i*6
		if !bytes.Equal(payload[off:off+6], mac) {
			t.Fatalf("repetition %d = % x, want % x", i, payload[off:off+6], mac)
		}
	}
}

// TestMacAddressPattern mirrors the validity check from bin/glpi-wakeonlan.
func TestMacAddressPattern(t *testing.T) {
	valid := []string{"01:02:03:04:05:06", "AA:bb:CC:dd:EE:ff"}
	invalid := []string{"", "01:02:03:04:05", "0102.0304.0506", "01-02-03-04-05-06", "gg:02:03:04:05:06"}
	for _, m := range valid {
		if !macAddressPattern.MatchString(m) {
			t.Errorf("%q should be valid", m)
		}
	}
	for _, m := range invalid {
		if macAddressPattern.MatchString(m) {
			t.Errorf("%q should be invalid", m)
		}
	}
}
