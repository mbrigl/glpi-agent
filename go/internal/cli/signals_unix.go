// SPDX-License-Identifier: GPL-2.0-only

//go:build !windows

package cli

import (
	"os"
	"os/signal"
	"syscall"
)

// notifyRunNow registers the "run now" signal (SIGUSR1, as the Perl daemon uses)
// on the channel.
func notifyRunNow(ch chan os.Signal) {
	signal.Notify(ch, syscall.SIGUSR1)
}
