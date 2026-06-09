// SPDX-License-Identifier: GPL-2.0-only

package agent

import (
	"bytes"
	"compress/zlib"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
	"github.com/glpi-project/glpi-agent/go/internal/transport"
)

func testLogger(t *testing.T) *logging.Logger {
	return logging.New(logging.Options{Backends: []string{"File"}, Logfile: filepath.Join(t.TempDir(), "log")})
}

func zlibReply(w http.ResponseWriter, body string) {
	w.Header().Set("Content-Type", "application/x-compress-zlib")
	zw := zlib.NewWriter(w)
	_, _ = zw.Write([]byte(body))
	_ = zw.Close()
}

// TestRunServerTargetReturnsExpiration checks the dialog returns the server's
// expiration (used by the daemon to schedule the next run) and submits both the
// contact and the inventory.
func TestRunServerTargetReturnsExpiration(t *testing.T) {
	var actions []string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		raw, _ := io.ReadAll(r.Body)
		zr, _ := zlib.NewReader(bytes.NewReader(raw))
		body, _ := io.ReadAll(zr)
		if bytes.Contains(body, []byte("contact")) {
			actions = append(actions, "contact")
			zlibReply(w, `{"status":"ok","expiration":"2h","tasks":{"inventory":{}}}`)
			return
		}
		actions = append(actions, "inventory")
		zlibReply(w, `{"status":"ok"}`)
	}))
	defer srv.Close()

	client, err := transport.NewGLPIClient(transport.GLPIOptions{AgentID: "uuid"})
	if err != nil {
		t.Fatal(err)
	}
	exp, err := RunServerTarget(testLogger(t), client, srv.URL, "dev-1", []byte(`{"action":"inventory"}`), "lab")
	if err != nil {
		t.Fatal(err)
	}
	if exp != 2*time.Hour {
		t.Errorf("expiration = %v, want 2h", exp)
	}
	if len(actions) != 2 || actions[0] != "contact" || actions[1] != "inventory" {
		t.Errorf("dialog = %v", actions)
	}
}

// TestRunServerTargetNotGLPI checks a non-GLPI answer yields ErrNotGLPIServer.
func TestRunServerTargetNotGLPI(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		zlibReply(w, `{"status":"ok"}`) // no expiration -> not a valid contact
	}))
	defer srv.Close()

	client, _ := transport.NewGLPIClient(transport.GLPIOptions{AgentID: "uuid"})
	_, err := RunServerTarget(testLogger(t), client, srv.URL, "dev-1", []byte(`{}`), "")
	if !errors.Is(err, ErrNotGLPIServer) {
		t.Errorf("err = %v, want ErrNotGLPIServer", err)
	}
}

// TestBuildInventory checks the document carries the device id and the tag.
func TestBuildInventory(t *testing.T) {
	now := time.Date(2026, 6, 9, 10, 0, 0, 0, time.UTC)
	inv := BuildInventory("host1", "lab", now)
	if inv.DeviceID != "host1-2026-06-09-10-00-00" {
		t.Errorf("deviceid = %q", inv.DeviceID)
	}
	acc, _ := inv.Content["ACCOUNTINFO"].(map[string]any)
	if acc["KEYVALUE"] != "lab" {
		t.Errorf("tag not set: %v", inv.Content["ACCOUNTINFO"])
	}
}
