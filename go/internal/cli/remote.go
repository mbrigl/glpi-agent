// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"flag"
	"fmt"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/content"
	"github.com/glpi-project/glpi-agent/go/internal/remote"
)

// runRemote implements the `remote` subcommand, derived from bin/glpi-remote and
// lib/GLPI/Agent/Task/RemoteInventory. Phase 7 covers the SSH path: it parses an
// ssh:// target, connects over golang.org/x/crypto/ssh and emits a remote
// inventory document. WinRM and the persistent remotes list are later work.
func runRemote(ctx *Context, args []string) int {
	stdout, stderr := ctx.Stdout, ctx.Stderr
	fs := flag.NewFlagSet("remote", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		target   = fs.String("target", "", "remote target, e.g. ssh://user:pass@host:22")
		user     = fs.String("user", "", "connection user (overrides the target userinfo)")
		password = fs.String("password", "", "connection password (overrides the target userinfo)")
		port     = fs.Int("port", 0, "connection port (default 22)")
		identity = fs.String("identity", "", "private key file for public-key auth")
		timeout  = fs.Int("timeout", 10, "connection timeout in seconds")
		hostKey  = fs.String("stricthostkeychecking", "", "host key policy: strict | accept-new | no")
		noCheck  = fs.Bool("no-check", false, "disable host key checking (alias for --stricthostkeychecking=no)")
		path     = fs.String("path", "-", "output path (\"-\" for stdout)")
		tag      = fs.String("tag", "", "administrative tag to add to the inventory")
		itemtype = fs.String("itemtype", "", "GLPI itemtype (default Computer)")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent remote --target ssh://user:pass@host[:port] [--identity <key>]")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if *target == "" {
		fmt.Fprintln(stderr, "no target given (--target ssh://...), aborting")
		return 2
	}

	cfg, err := sshConfigFromTarget(*target)
	if err != nil {
		fmt.Fprintf(stderr, "%v\n", err)
		return 2
	}
	// Explicit flags override the target userinfo.
	if *user != "" {
		cfg.User = *user
	}
	if *password != "" {
		cfg.Password = *password
	}
	if *port != 0 {
		cfg.Port = *port
	}
	cfg.IdentityFile = *identity
	cfg.Timeout = time.Duration(*timeout) * time.Second
	cfg.HostKeyChecking = *hostKey
	if *noCheck {
		cfg.HostKeyChecking = "no"
	}

	ctx.Logger.Info("connecting to remote " + cfg.Host)
	client, err := remote.Dial(cfg)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	defer client.Close()

	itemType := firstNonEmptyStr(*itemtype, ctx.Cfg.String("itemtype"))
	tagValue := firstNonEmptyStr(*tag, ctx.Cfg.String("tag"))
	inv, err := client.CollectInventory(itemType, tagValue, cfg.Host)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}

	if err := writeInventories([]*content.Inventory{inv}, *path, stdout); err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	return 0
}

// sshConfigFromTarget parses an ssh:// URL into a remote.Config, mirroring the
// target parsing of bin/glpi-remote.
func sshConfigFromTarget(target string) (remote.Config, error) {
	if !strings.Contains(target, "://") {
		target = "ssh://" + target
	}
	u, err := url.Parse(target)
	if err != nil {
		return remote.Config{}, fmt.Errorf("invalid target %q: %w", target, err)
	}
	if u.Scheme != "ssh" {
		return remote.Config{}, fmt.Errorf("unsupported scheme %q (only ssh is implemented)", u.Scheme)
	}
	cfg := remote.Config{Host: u.Hostname()}
	if p := u.Port(); p != "" {
		cfg.Port, _ = strconv.Atoi(p)
	}
	if u.User != nil {
		cfg.User = u.User.Username()
		if pw, ok := u.User.Password(); ok {
			cfg.Password = pw
		}
	}
	return cfg, nil
}
