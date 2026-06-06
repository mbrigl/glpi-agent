// SPDX-License-Identifier: GPL-2.0-only

// Package cli parses arguments and dispatches to subcommands.
//
// The upstream Perl agent ships one executable per task (bin/glpi-agent,
// bin/glpi-inventory, bin/glpi-injector, bin/glpi-netdiscovery, ...). The Go
// track consolidates them into a single binary with subcommands of the same
// names; each subcommand's behaviour and flags are derived from the matching
// Perl bin/ script. The top-level --version output mirrors bin/glpi-agent.
//
// Global options recognised before the subcommand name (e.g. --debug,
// --conf-file) build the layered configuration and the logger, mirroring the
// way bin/glpi-agent parses agent-wide options up front.
package cli

import (
	"fmt"
	"io"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/config"
	"github.com/glpi-project/glpi-agent/go/internal/logging"
	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// Context carries the resolved configuration, the logger and the output streams
// to a subcommand.
type Context struct {
	Cfg    *config.Config
	Logger *logging.Logger
	Stdout io.Writer
	Stderr io.Writer
}

// subcommand is one dispatchable command. run receives the context and the args
// after the subcommand name.
type subcommand struct {
	name    string
	summary string
	run     func(ctx *Context, args []string) int
}

// subcommands lists every command, mirroring the upstream bin/ executables.
// Commands not yet implemented return a clear "not implemented" status so the
// surface is complete from Phase 1 (same approach as the Rust track's skeleton).
func subcommands() []subcommand {
	return []subcommand{
		{"inventory", "run a local inventory (bin/glpi-inventory)", runInventory},
		{"inject", "push an inventory file to a server (bin/glpi-injector)", runInject},
		{"wakeonlan", "send a Wake-on-LAN magic packet (bin/glpi-wakeonlan)", runWakeOnLan},
		{"netdiscovery", "scan networks for devices (bin/glpi-netdiscovery)", runNetDiscovery},
		{"netinventory", "SNMP inventory of network devices (bin/glpi-netinventory)", notImplemented("netinventory")},
		{"esx", "inventory VMware ESX/vCenter (bin/glpi-esx)", runESX},
		{"remote", "remote inventory over SSH/WinRM (bin/glpi-remote)", runRemote},
	}
}

// Run is the entry point. It handles the top-level --version/--help, parses any
// leading global options into the configuration and logger, then dispatches to
// a subcommand.
func Run(args []string, stdout, stderr io.Writer) int {
	globals, rest := splitGlobals(args)

	if len(rest) == 0 {
		usage(stderr)
		return 2
	}

	switch rest[0] {
	case "--version", "-V", "version":
		for _, line := range version.VersionLines() {
			fmt.Fprintln(stdout, line)
		}
		return 0
	case "--help", "-h", "help":
		usage(stdout)
		return 0
	}

	cfg, err := buildConfig(globals)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 2
	}
	ctx := &Context{
		Cfg:    cfg,
		Logger: logging.New(cfg.LoggerOptions()),
		Stdout: stdout,
		Stderr: stderr,
	}

	for _, sc := range subcommands() {
		if sc.name == rest[0] {
			return sc.run(ctx, rest[1:])
		}
	}

	fmt.Fprintf(stderr, "unknown command %q\n\n", rest[0])
	usage(stderr)
	return 2
}

// buildConfig assembles the layered configuration: defaults, then a
// configuration file (--conf-file/--config), then the leading global options,
// then the _checkContent normalisation. Mirrors GLPI::Agent::Config->new.
func buildConfig(globals map[string]any) (*config.Config, error) {
	cfg := config.New()
	if f, ok := globals["conf-file"].(string); ok && f != "" {
		if err := cfg.LoadFile(f); err != nil {
			return nil, err
		}
		delete(globals, "conf-file")
	}
	delete(globals, "config") // backend selector; only "file" supported in Phase 1
	cfg.Apply(globals)
	if err := cfg.Check(); err != nil {
		return nil, err
	}
	return cfg, nil
}

// splitGlobals consumes leading "--key[=value]" tokens (agent-wide options) up
// to the subcommand name. A bare "--key" with no value is treated as a repeated
// count for --debug and a boolean otherwise.
func splitGlobals(args []string) (map[string]any, []string) {
	globals := map[string]any{}
	i := 0
	for ; i < len(args); i++ {
		arg := args[i]
		// The top-level version/help forms are commands, not global options,
		// even though they start with "--": leave them for the dispatcher.
		if arg == "--version" || arg == "-V" || arg == "--help" || arg == "-h" {
			break
		}
		if !strings.HasPrefix(arg, "--") {
			break // subcommand name reached
		}
		key, val, hasVal := strings.Cut(strings.TrimPrefix(arg, "--"), "=")
		switch {
		case key == "debug" && !hasVal:
			globals["debug"] = asInt(globals["debug"]) + 1
		case hasVal:
			globals[key] = val
		default:
			globals[key] = true
		}
	}
	return globals, args[i:]
}

func asInt(v any) int {
	if n, ok := v.(int); ok {
		return n
	}
	return 0
}

// notImplemented returns a runner that reports the command is not yet
// implemented, keeping the dispatch surface complete.
func notImplemented(name string) func(*Context, []string) int {
	return func(ctx *Context, _ []string) int {
		fmt.Fprintf(ctx.Stderr, "%s: not implemented yet\n", name)
		return 1
	}
}

func usage(w io.Writer) {
	fmt.Fprintf(w, "%s\n\n", version.String())
	fmt.Fprintln(w, "Usage: glpi-agent [global options] <command> [options]")
	fmt.Fprintln(w, "       glpi-agent --version")
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Global options: --debug, --color, --logger=<list>, --logfile=<path>, --conf-file=<path>")
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Commands:")
	for _, sc := range subcommands() {
		fmt.Fprintf(w, "  %-13s %s\n", sc.name, sc.summary)
	}
}
