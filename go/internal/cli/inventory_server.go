// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/glpi-project/glpi-agent/go/internal/agent"
)

// sendToServer performs the GLPI server dialog for one inventory (CONTACT +
// submit) and maps the outcome to a process exit code. The reusable dialog lives
// in internal/agent (shared with the daemon).
func sendToServer(ctx *Context, serverURL, deviceID string, inventoryJSON []byte, tag string) int {
	stderr := ctx.Stderr

	srv, client, _, err := newServerClient(ctx, serverURL)
	if err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}

	if _, err := agent.RunServerTarget(ctx.Logger, client, srv.URL, deviceID, inventoryJSON, tag); err != nil {
		fmt.Fprintf(stderr, "ERROR: %v\n", err)
		return 1
	}
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
