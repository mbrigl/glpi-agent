// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"path/filepath"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/state"
	"github.com/glpi-project/glpi-agent/go/internal/target"
	"github.com/glpi-project/glpi-agent/go/internal/transport"
	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// glpiOptionsFromConfig maps the global config (server/TLS/auth/proxy) to the
// GLPI client options, shared by the inventory command and the daemon.
func glpiOptionsFromConfig(ctx *Context, agentID string) transport.GLPIOptions {
	return transport.GLPIOptions{
		AgentID:       agentID,
		UserAgent:     version.AgentString(),
		Timeout:       time.Duration(ctx.Cfg.Int("timeout")) * time.Second,
		NoCompression: ctx.Cfg.Bool("no-compression"),
		NoSSLCheck:    ctx.Cfg.Bool("no-ssl-check"),
		CACertFile:    ctx.Cfg.String("ca-cert-file"),
		CACertDir:     ctx.Cfg.String("ca-cert-dir"),
		SSLCertFile:   ctx.Cfg.String("ssl-cert-file"),
		SSLKeyFile:    ctx.Cfg.String("ssl-key-file"),
		User:          ctx.Cfg.String("user"),
		Password:      ctx.Cfg.String("password"),
		Proxy:         ctx.Cfg.String("proxy"),
	}
}

// newServerClient resolves a server target, loads or creates its persistent
// agent id under the per-server vardir, and builds a GLPI client for it.
func newServerClient(ctx *Context, serverURL string) (*target.Server, *transport.GLPIClient, error) {
	srv, err := target.NewServer(serverURL)
	if err != nil {
		return nil, nil, err
	}
	st, err := state.LoadOrCreate(filepath.Join(agentVarDir(ctx), srv.Subdir()))
	if err != nil {
		return nil, nil, err
	}
	client, err := transport.NewGLPIClient(glpiOptionsFromConfig(ctx, st.AgentID))
	if err != nil {
		return nil, nil, err
	}
	return srv, client, nil
}
