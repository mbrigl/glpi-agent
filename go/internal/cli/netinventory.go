// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"encoding/json"
	"flag"
	"fmt"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/discovery"
)

// runNetInventory implements the `netinventory` subcommand, derived from
// bin/glpi-netinventory and lib/GLPI/Agent/Task/NetInventory.pm. Phase 3 covers
// the generic device properties plus the IF-MIB PORTS table over SNMP v1/v2c;
// the sysObjectID/MibSupport classification tail is follow-on.
func runNetInventory(ctx *Context, args []string) int {
	stdout, stderr := ctx.Stdout, ctx.Stderr
	fs := flag.NewFlagSet("netinventory", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		host      = fs.String("host", "", "device IP/hostname to inventory")
		community = fs.String("community", "public", "SNMP v1/v2c community")
		version   = fs.String("snmp-version", "2c", "SNMP version: 1 | 2c")
		port      = fs.Int("port", 161, "SNMP UDP port")
		timeout   = fs.Int("timeout", 5, "SNMP timeout in seconds")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent netinventory --host <ip> [--community public]")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if *host == "" {
		fmt.Fprintln(stderr, "no host given (--host), aborting")
		return 2
	}

	cred := discovery.Credential{ID: 1, Version: *version, Community: *community}
	ctx.Logger.Info("SNMP inventory of " + *host)
	getter, err := discovery.Dial(*host, uint16(*port), cred, time.Duration(*timeout)*time.Second)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	defer getter.Close()

	device, err := discovery.GetInventory(*host, getter)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	if device == nil {
		fmt.Fprintln(stderr, "host did not respond to SNMP")
		return 1
	}

	data, err := json.MarshalIndent(map[string]any{"DEVICE": device}, "", "   ")
	if err != nil {
		fmt.Fprintf(stderr, "failed to encode result: %v\n", err)
		return 1
	}
	fmt.Fprintln(stdout, string(data))
	return 0
}
