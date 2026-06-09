// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"flag"
	"fmt"
	"os"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/agent"
)

// runInventory implements the `inventory` subcommand, derived from
// bin/glpi-inventory and lib/GLPI/Agent/Task/Inventory.pm.
//
// Phase 1 emits a minimal but valid GLPI Agent Protocol inventory document
// (deviceid + action + itemtype + content with VERSIONCLIENT and the default
// HARDWARE.VMSYSTEM). Category collectors are added in Phase 6.
func runInventory(ctx *Context, args []string) int {
	stdout, stderr := ctx.Stdout, ctx.Stderr
	fs := flag.NewFlagSet("inventory", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		out  = fs.String("local", "-", "write inventory to this path (\"-\" for stdout)")
		tag  = fs.String("tag", "", "administrative tag to add to the inventory")
		name = fs.String("assetname", "", "asset name for the device id (default: hostname)")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent inventory [--local <path>] [--tag <tag>]")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}

	assetName := *name
	if assetName == "" {
		host, err := os.Hostname()
		if err != nil || host == "" {
			host = "localhost"
		}
		assetName = host
	}

	ctx.Logger.Debug("running local inventory for " + assetName)

	// The --tag flag overrides the configured tag, otherwise fall back to the
	// config value (tag is a config option in the Perl agent).
	tagValue := *tag
	if tagValue == "" {
		tagValue = ctx.Cfg.String("tag")
	}

	inv := agent.BuildInventory(assetName, tagValue, time.Now())

	data, err := inv.JSON()
	if err != nil {
		fmt.Fprintf(stderr, "failed to encode inventory: %v\n", err)
		return 1
	}

	// When a server is configured (a global option), send the inventory there
	// via the GLPI protocol instead of writing it locally.
	if server := ctx.Cfg.String("server"); server != "" {
		return sendToServer(ctx, server, inv.DeviceID, data, tagValue)
	}

	data = append(data, '\n')
	if *out == "-" || *out == "" {
		_, _ = stdout.Write(data)
		return 0
	}
	if err := os.WriteFile(*out, data, 0o644); err != nil {
		fmt.Fprintf(stderr, "failed to write %s: %v\n", *out, err)
		return 1
	}
	return 0
}
