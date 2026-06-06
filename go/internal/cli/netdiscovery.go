// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"encoding/json"
	"flag"
	"fmt"
	"strings"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/discovery"
)

// runNetDiscovery implements the `netdiscovery` subcommand, derived from
// bin/glpi-netdiscovery and lib/GLPI/Agent/Task/NetDiscovery.pm. Phase 2 covers
// the SNMP probe of an IP range with v1/v2c community auth and the generic
// device properties; SNMPv3, the threaded scan and sysObjectID classification
// are follow-on.
func runNetDiscovery(ctx *Context, args []string) int {
	stdout, stderr := ctx.Stdout, ctx.Stderr
	fs := flag.NewFlagSet("netdiscovery", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		ranges    = fs.String("range", "", "IPv4 range(s) to scan: CIDR, a-b, or single IP (comma-separated)")
		community = fs.String("community", "public", "SNMP v1/v2c community")
		version   = fs.String("snmp-version", "2c", "SNMP version: 1 | 2c")
		port      = fs.Int("port", 161, "SNMP UDP port")
		timeout   = fs.Int("timeout", 1, "per-host SNMP timeout in seconds")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent netdiscovery --range <CIDR|a-b|ip> [--community public]")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if *ranges == "" {
		fmt.Fprintln(stderr, "no range given (--range), aborting")
		return 2
	}

	cred := discovery.Credential{ID: 1, Version: *version, Community: *community}
	to := time.Duration(*timeout) * time.Second
	dial := func(host string) (discovery.SNMPGetter, error) {
		return discovery.Dial(host, uint16(*port), cred, to)
	}

	specs := splitNonEmpty(*ranges)
	ctx.Logger.Info(fmt.Sprintf("scanning %d range(s) over SNMP", len(specs)))
	devices, err := discovery.Scan(specs, dial)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 2
	}
	ctx.Logger.Info(fmt.Sprintf("found %d SNMP device(s)", len(devices)))

	out := map[string]any{"DEVICES": devices}
	data, err := json.MarshalIndent(out, "", "   ")
	if err != nil {
		fmt.Fprintf(stderr, "failed to encode result: %v\n", err)
		return 1
	}
	fmt.Fprintln(stdout, string(data))
	return 0
}

func splitNonEmpty(s string) []string {
	var out []string
	for _, p := range strings.Split(s, ",") {
		if p = strings.TrimSpace(p); p != "" {
			out = append(out, p)
		}
	}
	return out
}
