// SPDX-License-Identifier: GPL-2.0-only

package config

import (
	"os"
	"path/filepath"
	"testing"
)

// TestDefaults checks a few representative values from %default.
func TestDefaults(t *testing.T) {
	c := New()
	if got := c.String("logger"); got != "Stderr" {
		t.Errorf("default logger = %q, want Stderr", got)
	}
	if got := c.Int("httpd-port"); got != 62354 {
		t.Errorf("default httpd-port = %d, want 62354", got)
	}
	if got := c.Int("timeout"); got != 180 {
		t.Errorf("default timeout = %d, want 180", got)
	}
}

// TestFileThenCLIPrecedence verifies defaults < file < CLI options.
func TestFileThenCLIPrecedence(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "agent.cfg")
	os.WriteFile(cfgPath, []byte(`
# sample
server = https://from-file/
tag = "lab"   # quoted, with comment
unknown-directive = 1
`), 0o644)

	c := New()
	if err := c.LoadFile(cfgPath); err != nil {
		t.Fatal(err)
	}
	if got := c.String("tag"); got != "lab" {
		t.Errorf("tag from file = %q, want lab", got)
	}
	// CLI overrides the file.
	c.Apply(map[string]any{"server": "https://from-cli/"})
	if err := c.Check(); err != nil {
		t.Fatal(err)
	}
	if got := c.List("server"); len(got) != 1 || got[0] != "https://from-cli/" {
		t.Errorf("server = %v, want [https://from-cli/]", got)
	}
}

// TestCheckLogfileImpliesFileBackend mirrors the _checkContent rule that a
// logfile adds the File backend to the logger list.
func TestCheckLogfileImpliesFileBackend(t *testing.T) {
	c := New()
	c.Apply(map[string]any{"logfile": "/var/log/glpi.log"})
	if err := c.Check(); err != nil {
		t.Fatal(err)
	}
	backends := c.List("logger")
	found := false
	for _, b := range backends {
		if b == "File" {
			found = true
		}
	}
	if !found {
		t.Errorf("logger backends = %v, want to include File", backends)
	}
}

// TestCheckCommaSplitAndClamp covers comma-splitting and conf-reload-interval
// clamping.
func TestCheckCommaSplitAndClamp(t *testing.T) {
	c := New()
	c.Apply(map[string]any{
		"no-category":          "printer,monitor,,software",
		"conf-reload-interval": "30", // below the 60 minimum
	})
	if err := c.Check(); err != nil {
		t.Fatal(err)
	}
	cats := c.List("no-category")
	if len(cats) != 3 {
		t.Errorf("no-category = %v, want 3 entries (empty dropped)", cats)
	}
	if got := c.Int("conf-reload-interval"); got != 60 {
		t.Errorf("conf-reload-interval = %d, want clamped to 60", got)
	}
}

// TestCheckAntagonisticCAOptions mirrors the die on ca-cert-file + ca-cert-dir.
func TestCheckAntagonisticCAOptions(t *testing.T) {
	c := New()
	c.Apply(map[string]any{"ca-cert-file": "/a", "ca-cert-dir": "/b"})
	if err := c.Check(); err == nil {
		t.Fatal("expected error for ca-cert-file + ca-cert-dir, got nil")
	}
}
