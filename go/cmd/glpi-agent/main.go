// SPDX-License-Identifier: GPL-2.0-only

// Command glpi-agent is the single-binary entry point for the Go GLPI agent.
// It dispatches to subcommands that mirror the upstream Perl bin/ executables
// (see internal/cli). The implementation is derived exclusively from the
// upstream Perl agent; see ../../UPSTREAM.md.
package main

import (
	"os"

	"github.com/glpi-project/glpi-agent/go/internal/cli"
)

func main() {
	os.Exit(cli.Run(os.Args[1:], os.Stdout, os.Stderr))
}
