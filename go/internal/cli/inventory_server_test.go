// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"bytes"
	"compress/zlib"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/glpi-project/glpi-agent/go/internal/config"
	"github.com/glpi-project/glpi-agent/go/internal/logging"
	"github.com/glpi-project/glpi-agent/go/internal/target"
)

// recordedRequest is one POST the fake GLPI server received.
type recordedRequest struct {
	agentID string
	action  string
	body    map[string]any
}

// fakeGLPIServer is an httptest server that records requests and replies with a
// (zlib-compressed) JSON message produced by reply(action).
func fakeGLPIServer(t *testing.T, reply func(action string) string) (*httptest.Server, *[]recordedRequest) {
	t.Helper()
	var got []recordedRequest
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		raw, _ := io.ReadAll(r.Body)
		if r.Header.Get("Content-Type") == "application/x-compress-zlib" {
			zr, err := zlib.NewReader(bytes.NewReader(raw))
			if err != nil {
				t.Errorf("request body not zlib: %v", err)
				http.Error(w, "bad", 400)
				return
			}
			raw, _ = io.ReadAll(zr)
		}
		var body map[string]any
		_ = json.Unmarshal(raw, &body)
		action, _ := body["action"].(string)
		got = append(got, recordedRequest{
			agentID: r.Header.Get("GLPI-Agent-ID"),
			action:  action,
			body:    body,
		})

		w.Header().Set("Content-Type", "application/x-compress-zlib")
		zw := zlib.NewWriter(w)
		_, _ = zw.Write([]byte(reply(action)))
		_ = zw.Close()
	}))
	return srv, &got
}

func testContext(t *testing.T, server, vardir string) (*Context, *bytes.Buffer) {
	t.Helper()
	cfg := config.New()
	cfg.Apply(map[string]any{"server": server, "vardir": vardir})
	var errb bytes.Buffer
	// Route the logger to a temp file so info logs don't pollute test output.
	logger := logging.New(logging.Options{
		Backends: []string{"File"},
		Logfile:  filepath.Join(t.TempDir(), "agent.log"),
	})
	ctx := &Context{
		Cfg:    cfg,
		Logger: logger,
		Stdout: io.Discard,
		Stderr: &errb,
	}
	return ctx, &errb
}

// TestSendToServerEndToEnd drives the CONTACT + submit dialog against the fake
// server and checks both requests, the agent-id header, and the persisted id.
func TestSendToServerEndToEnd(t *testing.T) {
	srv, got := fakeGLPIServer(t, func(action string) string {
		if action == "contact" {
			return `{"status":"ok","expiration":"24h","tasks":{"inventory":{"server":"glpi"}}}`
		}
		return `{"status":"ok"}`
	})
	defer srv.Close()

	vardir := t.TempDir()
	ctx, errb := testContext(t, srv.URL, vardir)

	inventoryJSON := []byte(`{"action":"inventory","deviceid":"dev-1","content":{}}`)
	code := sendToServer(ctx, srv.URL, "dev-1", inventoryJSON, "lab")
	if code != 0 {
		t.Fatalf("exit = %d, stderr=%s", code, errb.String())
	}

	if len(*got) != 2 {
		t.Fatalf("server got %d requests, want 2 (contact + inventory)", len(*got))
	}
	contact, inv := (*got)[0], (*got)[1]
	if contact.action != "contact" || inv.action != "inventory" {
		t.Errorf("actions = %q, %q", contact.action, inv.action)
	}
	if contact.body["deviceid"] != "dev-1" || contact.body["tag"] != "lab" {
		t.Errorf("contact body = %v", contact.body)
	}
	if contact.agentID == "" || contact.agentID != inv.agentID {
		t.Errorf("agent id header inconsistent: %q vs %q", contact.agentID, inv.agentID)
	}

	// The agent id must be persisted under the per-server vardir.
	sub, _ := target.NewServer(srv.URL)
	statePath := filepath.Join(vardir, sub.Subdir(), "agent-state.json")
	data, err := os.ReadFile(statePath)
	if err != nil {
		t.Fatalf("agent state not persisted: %v", err)
	}
	var st struct {
		AgentID string `json:"agentid"`
	}
	_ = json.Unmarshal(data, &st)
	if st.AgentID != contact.agentID {
		t.Errorf("persisted agent id %q != sent %q", st.AgentID, contact.agentID)
	}
}

// TestSendToServerNotGLPI checks a server that doesn't return a valid contact
// (no expiration) is reported as not speaking the modern protocol.
func TestSendToServerNotGLPI(t *testing.T) {
	srv, got := fakeGLPIServer(t, func(string) string { return `{"status":"ok"}` }) // no expiration
	defer srv.Close()

	ctx, errb := testContext(t, srv.URL, t.TempDir())
	code := sendToServer(ctx, srv.URL, "dev-1", []byte(`{}`), "")
	if code == 0 {
		t.Error("expected a non-zero exit for a non-GLPI server")
	}
	if !bytes.Contains(errb.Bytes(), []byte("modern GLPI")) {
		t.Errorf("stderr = %q, want the not-modern-GLPI message", errb.String())
	}
	// Only the CONTACT was attempted; no inventory submission.
	if len(*got) != 1 {
		t.Errorf("got %d requests, want only the contact", len(*got))
	}
}

// TestSendToServerInventoryDisabled checks that when the server's task map omits
// inventory, no inventory is submitted but the run still succeeds.
func TestSendToServerInventoryDisabled(t *testing.T) {
	srv, got := fakeGLPIServer(t, func(string) string {
		return `{"status":"ok","expiration":"24h","tasks":{"netdiscovery":{}}}`
	})
	defer srv.Close()

	ctx, _ := testContext(t, srv.URL, t.TempDir())
	if code := sendToServer(ctx, srv.URL, "dev-1", []byte(`{}`), ""); code != 0 {
		t.Errorf("exit = %d, want 0", code)
	}
	if len(*got) != 1 {
		t.Errorf("got %d requests, want only the contact (inventory disabled)", len(*got))
	}
}
