// SPDX-License-Identifier: GPL-2.0-only

// Package cli parses arguments and dispatches to subcommands.
//
// The upstream Perl agent ships one executable per task (bin/glpi-agent,
// bin/glpi-inventory, bin/glpi-injector, bin/glpi-netdiscovery, ...). The Go
// track consolidates them into a single binary with subcommands of the same
// names; each subcommand's behaviour and flags are derived from the matching
// Perl bin/ script. The top-level --version output mirrors bin/glpi-agent.
package cli

import (
	"fmt"
	"io"

	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// subcommand is one dispatchable command. run receives the args after the
// subcommand name.
type subcommand struct {
	name    string
	summary string
	run     func(args []string, stdout, stderr io.Writer) int
}

// subcommands lists every command, mirroring the upstream bin/ executables.
// Commands not yet implemented return a clear "not implemented" status so the
// surface is complete from Phase 1 (same approach as the Rust track's skeleton).
func subcommands() []subcommand {
	return []subcommand{
		{"inventory", "run a local inventory (bin/glpi-inventory)", runInventory},
		{"inject", "push an inventory file to a server (bin/glpi-injector)", runInject},
		{"wakeonlan", "send a Wake-on-LAN magic packet (bin/glpi-wakeonlan)", runWakeOnLan},
		{"netdiscovery", "scan networks for devices (bin/glpi-netdiscovery)", notImplemented("netdiscovery")},
		{"netinventory", "SNMP inventory of network devices (bin/glpi-netinventory)", notImplemented("netinventory")},
		{"esx", "inventory VMware ESX/vCenter (bin/glpi-esx)", notImplemented("esx")},
		{"remote", "remote inventory over SSH/WinRM (bin/glpi-remote)", notImplemented("remote")},
	}
}

// Run is the entry point. It handles the top-level --version/--help, then
// dispatches to a subcommand.
func Run(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		usage(stderr)
		return 2
	}

	switch args[0] {
	case "--version", "-V", "version":
		for _, line := range version.VersionLines() {
			fmt.Fprintln(stdout, line)
		}
		return 0
	case "--help", "-h", "help":
		usage(stdout)
		return 0
	}

	for _, sc := range subcommands() {
		if sc.name == args[0] {
			return sc.run(args[1:], stdout, stderr)
		}
	}

	fmt.Fprintf(stderr, "unknown command %q\n\n", args[0])
	usage(stderr)
	return 2
}

// notImplemented returns a runner that reports the command is not yet
// implemented, keeping the dispatch surface complete.
func notImplemented(name string) func([]string, io.Writer, io.Writer) int {
	return func(_ []string, _ io.Writer, stderr io.Writer) int {
		fmt.Fprintf(stderr, "%s: not implemented yet\n", name)
		return 1
	}
}

func usage(w io.Writer) {
	fmt.Fprintf(w, "%s\n\n", version.String())
	fmt.Fprintln(w, "Usage: glpi-agent <command> [options]")
	fmt.Fprintln(w, "       glpi-agent --version")
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Commands:")
	for _, sc := range subcommands() {
		fmt.Fprintf(w, "  %-13s %s\n", sc.name, sc.summary)
	}
}
