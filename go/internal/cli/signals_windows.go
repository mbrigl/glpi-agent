// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package cli

import "os"

// notifyRunNow is a no-op on Windows, which has no SIGUSR1 "run now" signal.
func notifyRunNow(ch chan os.Signal) {}
