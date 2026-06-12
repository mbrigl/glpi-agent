// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"flag"
	"fmt"
	"os"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/collect"
	"github.com/glpi-project/glpi-agent/go/internal/content"
	"github.com/glpi-project/glpi-agent/go/internal/transport"
	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// runCollect implements the `collect` subcommand, derived from
// lib/GLPI/Agent/Task/Collect.pm: a server-driven task that fetches collection
// jobs (findFile / getFromRegistry / getFromWMI) over the Fusion plugin protocol
// and posts their results back. Requires a server target.
func runCollect(ctx *Context, args []string) int {
	stderr := ctx.Stderr
	fs := flag.NewFlagSet("collect", flag.ContinueOnError)
	fs.SetOutput(stderr)
	server := fs.String("server", "", "GLPI server URL (defaults to the configured server)")
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent collect --server <url>")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}

	serverURL := *server
	if serverURL == "" {
		serverURL = ctx.Cfg.String("server")
	}
	if serverURL == "" {
		fmt.Fprintln(stderr, "collect: a --server URL is required (Collect is a server-driven task)")
		return 2
	}

	// Resolve the server target + persistent agent id (reuses the TLS/auth/proxy
	// config), then build a Fusion client for the plugin protocol.
	srv, _, st, err := newServerClient(ctx, serverURL)
	if err != nil {
		fmt.Fprintf(stderr, "collect: %v\n", err)
		return 1
	}
	opts := glpiOptionsFromConfig(ctx, st.AgentID)
	opts.UserAgent = version.AgentString()
	fusion, err := transport.NewFusionClient(opts)
	if err != nil {
		fmt.Fprintf(stderr, "collect: %v\n", err)
		return 1
	}

	deviceID := content.DeviceID(hostnameOr("localhost"), time.Now())
	task := collect.NewTask(ctx.Logger, deviceID, collect.DefaultModules()...)
	if err := task.Run(fusion, srv.URL); err != nil {
		fmt.Fprintf(stderr, "collect: %v\n", err)
		return 1
	}
	return 0
}

// hostnameOr returns the host name or a fallback.
func hostnameOr(fallback string) string {
	if h, err := os.Hostname(); err == nil && h != "" {
		return h
	}
	return fallback
}
