// SPDX-License-Identifier: GPL-2.0-only

//go:build !linux

package cli

import "fmt"

// sendMagicPacketEthernet is only implemented on Linux (raw AF_PACKET sockets);
// elsewhere the udp method should be used.
func sendMagicPacketEthernet(mac string) error {
	return fmt.Errorf("ethernet method is only supported on Linux (use the udp method)")
}
