// SPDX-License-Identifier: GPL-2.0-only

package collect

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

func testLogger() *logging.Logger { return logging.New(logging.Options{}) }

// TestFileCollector exercises the findFile filters: recursion, name/iname, size
// and SHA256 checksum, plus the result limit.
func TestFileCollector(t *testing.T) {
	dir := t.TempDir()
	mustWrite(t, filepath.Join(dir, "a.txt"), "hello")
	mustWrite(t, filepath.Join(dir, "b.log"), "world!!")
	sub := filepath.Join(dir, "sub")
	if err := os.Mkdir(sub, 0o755); err != nil {
		t.Fatal(err)
	}
	mustWrite(t, filepath.Join(sub, "c.txt"), "deep")

	fc := FileCollector{}

	// Non-recursive: only the two top-level files (filter is_file).
	res := fc.Results(map[string]any{
		"dir": dir, "limit": float64(100), "recursive": false,
		"filter": map[string]any{"is_file": true},
	}, testLogger())
	if len(res) != 2 {
		t.Fatalf("non-recursive: got %d files, want 2", len(res))
	}

	// Recursive: includes sub/c.txt.
	res = fc.Results(map[string]any{
		"dir": dir, "limit": float64(100), "recursive": true,
		"filter": map[string]any{"is_file": true},
	}, testLogger())
	if len(res) != 3 {
		t.Fatalf("recursive: got %d files, want 3", len(res))
	}

	// Exact name filter.
	res = fc.Results(map[string]any{
		"dir": dir, "limit": float64(100), "recursive": true,
		"filter": map[string]any{"is_file": true, "name": "c.txt"},
	}, testLogger())
	if len(res) != 1 || filepath.Base(res[0]["path"].(string)) != "c.txt" {
		t.Fatalf("name filter: got %v", res)
	}

	// Size filter (exactly 7 bytes -> b.log "world!!").
	res = fc.Results(map[string]any{
		"dir": dir, "limit": float64(100), "recursive": true,
		"filter": map[string]any{"is_file": true, "sizeEquals": float64(7)},
	}, testLogger())
	if len(res) != 1 || res[0]["size"].(int64) != 7 {
		t.Fatalf("size filter: got %v", res)
	}

	// SHA256 checksum filter for "hello".
	sum := sha256.Sum256([]byte("hello"))
	res = fc.Results(map[string]any{
		"dir": dir, "limit": float64(100), "recursive": true,
		"filter": map[string]any{"is_file": true, "checkSumSHA2": hex.EncodeToString(sum[:])},
	}, testLogger())
	if len(res) != 1 || filepath.Base(res[0]["path"].(string)) != "a.txt" {
		t.Fatalf("sha256 filter: got %v", res)
	}

	// Limit caps the results.
	res = fc.Results(map[string]any{
		"dir": dir, "limit": float64(1), "recursive": true,
		"filter": map[string]any{"is_file": true},
	}, testLogger())
	if len(res) != 1 {
		t.Fatalf("limit: got %d, want 1", len(res))
	}
}

func mustWrite(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
