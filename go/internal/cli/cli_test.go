// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"bytes"
	"strings"
	"testing"
)

// TestRunVersion guards the top-level --version output (a Phase 1 deliverable):
// it must print the version string and exit 0, not fall through to usage. This
// regressed once when global-option parsing was added, so it is pinned here.
func TestRunVersion(t *testing.T) {
	for _, arg := range []string{"--version", "-V", "version"} {
		var out, errOut bytes.Buffer
		code := Run([]string{arg}, &out, &errOut)
		if code != 0 {
			t.Errorf("Run(%q) exit = %d, want 0 (stderr: %s)", arg, code, errOut.String())
		}
		if !strings.Contains(out.String(), "GLPI Agent (2.17.0)") {
			t.Errorf("Run(%q) stdout = %q, want it to contain the version string", arg, out.String())
		}
		if strings.Contains(out.String(), "Usage:") {
			t.Errorf("Run(%q) printed usage instead of the version", arg)
		}
	}
}

// TestRunUnknownCommand checks an unknown command is rejected with exit 2.
func TestRunUnknownCommand(t *testing.T) {
	var out, errOut bytes.Buffer
	if code := Run([]string{"nope"}, &out, &errOut); code != 2 {
		t.Errorf("Run(nope) exit = %d, want 2", code)
	}
}

// TestGlobalDebugThenSubcommand verifies a leading global option is consumed and
// the subcommand still dispatches.
func TestGlobalDebugThenSubcommand(t *testing.T) {
	var out, errOut bytes.Buffer
	// Reaching netdiscovery's argument validation (no --range -> exit 2) proves
	// dispatch worked past the leading global --debug.
	code := Run([]string{"--debug", "netdiscovery"}, &out, &errOut)
	if code != 2 {
		t.Errorf("exit = %d, want 2 from netdiscovery arg validation", code)
	}
	if !strings.Contains(errOut.String(), "no range given") {
		t.Errorf("stderr = %q, want the netdiscovery validation message", errOut.String())
	}
}
