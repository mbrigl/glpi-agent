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

// TestWoLEthernetFrame checks the raw L2 frame header (dst+src+ethertype 0x0842),
// the ethernet method of WakeOnLan.pm.
func TestWoLEthernetFrame(t *testing.T) {
	dst := []byte{0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff}
	src := []byte{0x11, 0x22, 0x33, 0x44, 0x55, 0x66}
	payload, _ := magicPayload("aa:bb:cc:dd:ee:ff")
	frame := wolEthernetFrame(dst, src, payload)

	if len(frame) != 14+len(payload) {
		t.Fatalf("frame len = %d, want %d", len(frame), 14+len(payload))
	}
	if !bytes.Equal(frame[0:6], dst) || !bytes.Equal(frame[6:12], src) {
		t.Errorf("frame addresses wrong: % x", frame[:12])
	}
	if frame[12] != 0x08 || frame[13] != 0x42 {
		t.Errorf("ethertype = %02x%02x, want 0842", frame[12], frame[13])
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
