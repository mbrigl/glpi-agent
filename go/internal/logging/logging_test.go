// SPDX-License-Identifier: GPL-2.0-only

package logging

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestVerbosityFromDebug mirrors the constructor mapping in Logger.pm.
func TestVerbosityFromDebug(t *testing.T) {
	cases := map[int]Level{0: LevelInfo, 1: LevelDebug, 2: LevelDebug2, 5: LevelDebug2}
	for debug, want := range cases {
		if got := VerbosityFromDebug(debug); got != want {
			t.Errorf("VerbosityFromDebug(%d) = %d, want %d", debug, got, want)
		}
	}
}

// TestLevelGating checks that messages below the verbosity are dropped and the
// Stderr format matches Logger/Stderr.pm ("[level] message").
func TestLevelGating(t *testing.T) {
	var buf bytes.Buffer
	l := &Logger{verbosity: LevelInfo, backends: []Backend{&stderrBackend{w: &buf}}}

	l.Debug("hidden")      // below INFO -> dropped
	l.Info("visible info") // shown
	l.Error("visible err") // shown
	l.Debug2("hidden too") // dropped

	out := buf.String()
	if strings.Contains(out, "hidden") {
		t.Errorf("debug message leaked at INFO verbosity:\n%s", out)
	}
	if !strings.Contains(out, "[info] visible info\n") {
		t.Errorf("missing info line, got:\n%s", out)
	}
	if !strings.Contains(out, "[error] visible err\n") {
		t.Errorf("missing error line, got:\n%s", out)
	}
}

// TestStderrColorFormat checks the exact ANSI format for an info message.
func TestStderrColorFormat(t *testing.T) {
	var buf bytes.Buffer
	b := &stderrBackend{color: true, w: &buf}
	b.AddMessage(LevelInfo, "hello")
	want := "\033[1;34m[info]\033[0m hello\n"
	if buf.String() != want {
		t.Errorf("colored info = %q, want %q", buf.String(), want)
	}
}

// TestFileBackend writes via a File backend and checks the
// "[localtime][level] message" shape from Logger/File.pm.
func TestFileBackend(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "agent.log")
	fb := &fileBackend{path: path}
	fb.AddMessage(LevelWarning, "disk almost full")

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	line := string(data)
	if !strings.Contains(line, "[warning] disk almost full") {
		t.Errorf("file line = %q, want it to contain [warning] disk almost full", line)
	}
	if !strings.HasPrefix(line, "[") {
		t.Errorf("file line should start with a [timestamp], got %q", line)
	}
}
