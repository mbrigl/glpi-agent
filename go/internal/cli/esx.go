// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"context"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/glpi-project/glpi-agent/go/internal/content"
	"github.com/glpi-project/glpi-agent/go/internal/vsphere"
)

// runESX implements the `esx` subcommand, derived from bin/glpi-esx and
// lib/GLPI/Agent/Task/ESX.pm. It connects to an ESX host or vCenter, collects
// one inventory per host and writes it to stdout, a file, or (for several hosts)
// one file per host in a directory. The transport is govmomi.
func runESX(ctx *Context, args []string) int {
	stdout, stderr := ctx.Stdout, ctx.Stderr
	fs := flag.NewFlagSet("esx", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		host       = fs.String("host", "", "ESX/vCenter host name")
		user       = fs.String("user", "", "connection user")
		password   = fs.String("password", "", "connection password")
		path       = fs.String("path", "-", "output path (\"-\" for stdout, or a directory)")
		tag        = fs.String("tag", "", "administrative tag to add to the inventory")
		itemtype   = fs.String("esx-itemtype", "", "GLPI itemtype for the host (default Computer)")
		noSSLCheck = fs.Bool("no-ssl-check", true, "do not verify the server certificate")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent esx --host <host> --user <user> --password <pw> [--path <dir|->]")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if *host == "" {
		fmt.Fprintln(stderr, "no host given (--host), aborting")
		return 2
	}
	if *user == "" {
		fmt.Fprintln(stderr, "no user provided for ESX connection")
		return 2
	}
	if *password == "" {
		fmt.Fprintln(stderr, "no password provided for ESX connection")
		return 2
	}

	tagValue := firstNonEmptyStr(*tag, ctx.Cfg.String("tag"))
	itemType := firstNonEmptyStr(*itemtype, ctx.Cfg.String("esx-itemtype"))

	c := context.Background()
	ctx.Logger.Info("connecting to ESX host " + *host)
	client, err := vsphere.Connect(c, *host, *user, *password, *noSSLCheck)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	defer client.Close(c)

	inventories, err := client.CollectHosts(c, itemType, tagValue)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	if len(inventories) == 0 {
		fmt.Fprintln(stderr, "no ESX host found")
		return 1
	}
	ctx.Logger.Info(fmt.Sprintf("collected %d ESX host(s)", len(inventories)))

	if err := writeInventories(inventories, *path, stdout); err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	return 0
}

// writeInventories sends the inventories to stdout ("-") or writes one
// <deviceid>.json file per host into the given directory, mirroring the path /
// stdout handling of bin/glpi-esx.
func writeInventories(inventories []*content.Inventory, path string, stdout interface{ Write([]byte) (int, error) }) error {
	for _, inv := range inventories {
		data, err := inv.JSON()
		if err != nil {
			return err
		}
		if path == "-" || path == "" {
			if _, err := stdout.Write(append(data, '\n')); err != nil {
				return err
			}
			continue
		}
		file := filepath.Join(path, inv.DeviceID+".json")
		if err := os.WriteFile(file, append(data, '\n'), 0o644); err != nil {
			return err
		}
	}
	return nil
}

func firstNonEmptyStr(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}
