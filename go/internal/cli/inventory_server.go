// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/protocol"
	"github.com/glpi-project/glpi-agent/go/internal/state"
	"github.com/glpi-project/glpi-agent/go/internal/target"
	"github.com/glpi-project/glpi-agent/go/internal/transport"
	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// sendToServer performs the GLPI server dialog for one inventory: a CONTACT
// handshake followed by the inventory submission, mirroring the GLPI-server
// branch of GLPI::Agent::getContact + Task/Inventory::submit. Only the modern
// GLPI protocol is supported (no legacy OCS fallback).
func sendToServer(ctx *Context, serverURL, deviceID string, inventoryJSON []byte, tag string) int {
	stderr := ctx.Stderr

	srv, err := target.NewServer(serverURL)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 2
	}

	// Per-server state directory holding the persistent agent id.
	varPath := filepath.Join(agentVarDir(ctx), srv.Subdir())
	st, err := state.LoadOrCreate(varPath)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}

	client, err := transport.NewGLPIClient(transport.GLPIOptions{
		AgentID:       st.AgentID,
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
	})
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}

	// 1) CONTACT handshake.
	contact := protocol.Contact{
		DeviceID:       deviceID,
		InstalledTasks: []string{"inventory"},
		EnabledTasks:   []string{"inventory"},
		Tag:            tag,
	}
	contactMsg, err := contact.Encode()
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
	ctx.Logger.Info("sending contact request to " + srv.URL)
	answer, err := client.Send(srv.URL, contactMsg)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: contact request failed: %v\n", err)
		return 1
	}
	if !answer.IsContactValid() {
		fmt.Fprintln(stderr, "ERROR: server does not speak the modern GLPI Agent protocol (legacy OCS is not supported)")
		return 1
	}
	if !answer.TaskEnabled("inventory") {
		ctx.Logger.Info("inventory task disabled by server, nothing to send")
		return 0
	}

	// 2) Inventory submission.
	ctx.Logger.Info("sending inventory to " + srv.URL)
	if _, err := client.Send(srv.URL, inventoryJSON); err != nil {
		fmt.Fprintf(stderr, "ERROR: inventory submission failed: %v\n", err)
		return 1
	}
	ctx.Logger.Info("inventory successfully sent to " + srv.URL)
	return 0
}

// agentVarDir returns the base directory for agent state: the configured vardir,
// or the OS user-config directory (falling back to the working directory).
func agentVarDir(ctx *Context) string {
	if v := ctx.Cfg.String("vardir"); v != "" {
		return v
	}
	if dir, err := os.UserConfigDir(); err == nil && dir != "" {
		return filepath.Join(dir, "glpi-agent")
	}
	return ".glpi-agent"
}
