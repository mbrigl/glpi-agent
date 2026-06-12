// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"encoding/hex"
	"flag"
	"fmt"
	"net"
	"regexp"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// macAddressPattern mirrors $mac_address_pattern in
// lib/GLPI/Agent/Tools/Network.pm: six colon-separated hex bytes.
var macAddressPattern = regexp.MustCompile(`^[0-9A-Fa-f]{2}(:[0-9A-Fa-f]{2}){5}$`)

// runWakeOnLan implements the `wakeonlan` subcommand, derived from
// bin/glpi-wakeonlan and lib/GLPI/Agent/Task/WakeOnLan.pm.
//
// Only the self-contained "udp" method is implemented in Phase 1; it mirrors
// _send_magic_packet_udp (broadcast to 255.255.255.255:9). The "ethernet"
// method needs raw layer-2 sockets and root and is deferred.
func runWakeOnLan(ctx *Context, args []string) int {
	stdout, stderr := ctx.Stdout, ctx.Stderr
	fs := flag.NewFlagSet("wakeonlan", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		mac     = fs.String("mac", "", "target MAC address")
		methods = fs.String("methods", "udp", "comma-separated methods to use (udp, ethernet)")
		showVer = fs.Bool("version", false, "print the task version and exit")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent wakeonlan --mac <MAC> [--methods udp]")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}

	if *showVer {
		// Mirrors the --version block of bin/glpi-wakeonlan.
		fmt.Fprintf(stdout, "WakeOnLan task, based on %s Agent v%s\n", version.Provider, version.Version)
		return 0
	}

	if *mac == "" {
		fmt.Fprintln(stderr, "no mac address given, aborting")
		return 2
	}
	if !macAddressPattern.MatchString(*mac) {
		fmt.Fprintln(stderr, "invalid mac address given, aborting")
		return 2
	}

	// Perl runs the listed methods in order, stopping at the first that works.
	for _, method := range strings.Split(*methods, ",") {
		method = strings.TrimSpace(method)
		switch method {
		case "udp":
			if err := sendMagicPacketUDP(*mac); err != nil {
				fmt.Fprintf(stderr, "Impossible to use udp method: %v\n", err)
				return 1
			}
			fmt.Fprintf(stdout, "Sent magic packet to %s as UDP packet\n", *mac)
			return 0
		case "ethernet":
			if err := sendMagicPacketEthernet(*mac); err != nil {
				fmt.Fprintf(stderr, "Impossible to use ethernet method: %v\n", err)
				return 1
			}
			fmt.Fprintf(stdout, "Sent magic packet to %s as ethernet frame\n", *mac)
			return 0
		default:
			fmt.Fprintf(stderr, "unknown method %q\n", method)
			return 2
		}
	}
	return 0
}

// sendMagicPacketUDP mirrors _send_magic_packet_udp in WakeOnLan.pm: a UDP
// broadcast of the magic packet to 255.255.255.255:9.
func sendMagicPacketUDP(mac string) error {
	payload, err := magicPayload(mac)
	if err != nil {
		return err
	}
	conn, err := net.DialUDP("udp4", nil, &net.UDPAddr{IP: net.IPv4bcast, Port: 9})
	if err != nil {
		return fmt.Errorf("can't open socket: %w", err)
	}
	defer conn.Close()
	if _, err := conn.Write(payload); err != nil {
		return fmt.Errorf("can't send packet: %w", err)
	}
	return nil
}

// magicPayload builds the Wake-on-LAN payload, mirroring _getPayload:
// six 0xFF bytes followed by the 6-byte MAC repeated 16 times.
func magicPayload(mac string) ([]byte, error) {
	hw, err := hex.DecodeString(strings.ReplaceAll(mac, ":", ""))
	if err != nil || len(hw) != 6 {
		return nil, fmt.Errorf("invalid mac address %q", mac)
	}
	payload := make([]byte, 0, 6+16*6)
	for i := 0; i < 6; i++ {
		payload = append(payload, 0xFF)
	}
	for i := 0; i < 16; i++ {
		payload = append(payload, hw...)
	}
	return payload, nil
}

// wolEthernetFrame builds the raw layer-2 magic-packet frame: dst MAC + src MAC
// + ethertype 0x0842 + payload (WakeOnLan.pm::_send_magic_packet_ethernet).
func wolEthernetFrame(dst, src, payload []byte) []byte {
	frame := make([]byte, 0, 14+len(payload))
	frame = append(frame, dst...)
	frame = append(frame, src...)
	frame = append(frame, 0x08, 0x42)
	frame = append(frame, payload...)
	return frame
}
