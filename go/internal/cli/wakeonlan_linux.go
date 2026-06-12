// SPDX-License-Identifier: GPL-2.0-only

//go:build linux

package cli

import (
	"encoding/hex"
	"fmt"
	"net"
	"strings"

	"golang.org/x/sys/unix"
)

// sendMagicPacketEthernet sends the Wake-on-LAN magic packet as a raw layer-2
// Ethernet frame on every usable interface, mirroring
// WakeOnLan.pm::_send_magic_packet_ethernet (dst + src MAC + ethertype 0x0842 +
// payload). Needs CAP_NET_RAW / root.
func sendMagicPacketEthernet(mac string) error {
	target, err := hex.DecodeString(strings.ReplaceAll(mac, ":", ""))
	if err != nil || len(target) != 6 {
		return fmt.Errorf("invalid mac address %q", mac)
	}
	payload, err := magicPayload(mac)
	if err != nil {
		return err
	}

	ifaces, err := net.Interfaces()
	if err != nil {
		return err
	}

	var sent int
	var lastErr error
	for _, iface := range ifaces {
		if iface.Flags&net.FlagLoopback != 0 || iface.Flags&net.FlagUp == 0 {
			continue
		}
		if len(iface.HardwareAddr) != 6 {
			continue
		}

		frame := wolEthernetFrame(target, iface.HardwareAddr, payload)
		if err := sendRawFrame(iface.Index, target, frame); err != nil {
			lastErr = err
			continue
		}
		sent++
	}

	if sent == 0 {
		if lastErr != nil {
			return fmt.Errorf("can't send ethernet frame: %w", lastErr)
		}
		return fmt.Errorf("no usable interface found")
	}
	return nil
}

// sendRawFrame opens an AF_PACKET raw socket bound to the given interface and
// sends one frame.
func sendRawFrame(ifindex int, dst, frame []byte) error {
	fd, err := unix.Socket(unix.AF_PACKET, unix.SOCK_RAW, int(htons(unix.ETH_P_ALL)))
	if err != nil {
		return err // EPERM when not privileged
	}
	defer unix.Close(fd)

	addr := &unix.SockaddrLinklayer{
		Protocol: htons(unix.ETH_P_ALL),
		Ifindex:  ifindex,
		Halen:    6,
	}
	copy(addr.Addr[:], dst)
	return unix.Sendto(fd, frame, 0, addr)
}

// htons converts a uint16 to network byte order.
func htons(v uint16) uint16 {
	return (v<<8)&0xff00 | v>>8
}
