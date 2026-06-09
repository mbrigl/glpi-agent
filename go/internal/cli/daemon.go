// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"context"
	"crypto/tls"
	"flag"
	"fmt"
	"net"
	"os"
	"os/signal"
	"strconv"
	"syscall"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/agent"
	"github.com/glpi-project/glpi-agent/go/internal/httpd"
	"github.com/glpi-project/glpi-agent/go/internal/scheduler"
)

// maxPoll caps how long the daemon sleeps between schedule checks, so a "run
// now" signal or shutdown is still reasonably responsive even with far-future
// next-run dates.
const maxPoll = time.Hour

// runDaemon implements the `daemon` subcommand: a foreground scheduling loop
// that periodically sends an inventory to each configured GLPI server,
// honouring the server's expiration and backing off on errors. Derived from the
// run-loop of GLPI::Agent::Daemon (without the Perl process machinery — fork,
// PID files, IPC, daemonize).
func runDaemon(ctx *Context, args []string) int {
	stderr := ctx.Stderr
	fs := flag.NewFlagSet("daemon", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		name = fs.String("assetname", "", "asset name for the device id (default: hostname)")
	)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent --server <url>[,<url>...] [--delaytime <s>] daemon")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}

	servers := splitNonEmpty(ctx.Cfg.String("server"))
	if len(servers) == 0 {
		fmt.Fprintln(stderr, "the daemon needs at least one --server, aborting")
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
	tag := ctx.Cfg.String("tag")

	// delaytime drives the initial run stagger, the nominal interval before the
	// first server contact, and the backoff cap (Config.pm default 3600s).
	delay := time.Duration(ctx.Cfg.Int("delaytime")) * time.Second

	targets, err := buildServerTargets(ctx, servers, assetName, tag, delay)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}

	ag := agent.NewAgent(ctx.Logger, targets)

	// Shutdown on SIGINT/SIGTERM, "run now" on SIGUSR1.
	rootCtx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Start the HTTP control server unless disabled.
	if !ctx.Cfg.Bool("no-httpd") {
		if err := startControlServer(rootCtx, ctx, ag); err != nil {
			fmt.Fprintf(stderr, "ERROR: %v\n", err)
			return 1
		}
	}
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-stop
		ctx.Logger.Info("received shutdown signal")
		cancel()
	}()
	runNow := make(chan os.Signal, 1)
	notifyRunNow(runNow) // SIGUSR1 on unix; no-op on Windows
	go func() {
		for range runNow {
			ctx.Logger.Info("received run-now signal")
			ag.RunNow()
		}
	}()

	ctx.Logger.Info(fmt.Sprintf("daemon started with %d server target(s)", len(targets)))
	ag.Loop(rootCtx, daemonSleeper(ag))
	return 0
}

// buildServerTargets creates one scheduled target per server URL.
func buildServerTargets(ctx *Context, servers []string, assetName, tag string, delay time.Duration) ([]*agent.ScheduledTarget, error) {
	var targets []*agent.ScheduledTarget
	for _, url := range servers {
		srv, client, err := newServerClient(ctx, url)
		if err != nil {
			return nil, err
		}
		serverURL := srv.URL
		targets = append(targets, &agent.ScheduledTarget{
			Name:  serverURL,
			Sched: scheduler.New(delay, delay),
			Run: func() (time.Duration, error) {
				inv := agent.BuildInventory(assetName, tag, time.Now())
				data, err := inv.JSON()
				if err != nil {
					return 0, err
				}
				return agent.RunServerTarget(ctx.Logger, client, serverURL, inv.DeviceID, data, tag)
			},
		})
	}
	return targets, nil
}

// startControlServer binds the HTTP control server to httpd-ip:httpd-port and
// serves it in the background until rootCtx is cancelled.
func startControlServer(rootCtx context.Context, ctx *Context, ag *agent.Agent) error {
	addr := net.JoinHostPort(ctx.Cfg.String("httpd-ip"), strconv.Itoa(ctx.Cfg.Int("httpd-port")))
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return fmt.Errorf("control server cannot listen on %s: %w", addr, err)
	}
	tlsConfig, err := controlServerTLS(ctx)
	if err != nil {
		return err
	}
	srv := httpd.New(ag, ctx.Logger, ctx.Cfg.List("httpd-trust"))
	scheme := "http"
	if tlsConfig != nil {
		scheme = "https"
	}
	ctx.Logger.Info("control server listening on " + scheme + "://" + ln.Addr().String())
	go func() {
		if err := srv.Serve(rootCtx, ln, tlsConfig); err != nil {
			ctx.Logger.Error("control server stopped: " + err.Error())
		}
	}()
	return nil
}

// controlServerTLS builds the TLS config for the control server from the
// httpd-ssl-cert-file / httpd-ssl-key-file options, or returns nil for plain
// HTTP when no server certificate is configured.
func controlServerTLS(ctx *Context) (*tls.Config, error) {
	certFile := ctx.Cfg.String("httpd-ssl-cert-file")
	keyFile := ctx.Cfg.String("httpd-ssl-key-file")
	if certFile == "" && keyFile == "" {
		return nil, nil
	}
	if certFile == "" || keyFile == "" {
		return nil, fmt.Errorf("both httpd-ssl-cert-file and httpd-ssl-key-file are required for HTTPS")
	}
	cert, err := tls.LoadX509KeyPair(certFile, keyFile)
	if err != nil {
		return nil, fmt.Errorf("loading control-server certificate: %w", err)
	}
	return &tls.Config{Certificates: []tls.Certificate{cert}, MinVersion: tls.VersionTLS12}, nil
}

// daemonSleeper sleeps until the earliest next-run time (capped by maxPoll),
// waking early when the agent is asked to run now (via RunNow, e.g. SIGUSR1) and
// returning false on context cancellation.
func daemonSleeper(ag *agent.Agent) func(context.Context) bool {
	return func(ctx context.Context) bool {
		wait := maxPoll
		now := time.Now()
		for _, t := range ag.Targets() {
			if d := t.NextRun.Sub(now); d < wait {
				wait = d
			}
		}
		if wait < 0 {
			wait = 0
		}
		timer := time.NewTimer(wait)
		defer timer.Stop()
		select {
		case <-ctx.Done():
			return false
		case <-ag.Wake():
			return true
		case <-timer.C:
			return true
		}
	}
}
