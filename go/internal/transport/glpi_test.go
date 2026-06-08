// SPDX-License-Identifier: GPL-2.0-only

package transport

import (
	"bytes"
	"compress/zlib"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
)

// readBody inflates a zlib request body when the content type says so.
func readBody(t *testing.T, r *http.Request) []byte {
	t.Helper()
	raw, _ := io.ReadAll(r.Body)
	if r.Header.Get("Content-Type") == "application/x-compress-zlib" {
		zr, err := zlib.NewReader(bytes.NewReader(raw))
		if err != nil {
			t.Fatalf("request body not zlib: %v", err)
		}
		raw, _ = io.ReadAll(zr)
	}
	return raw
}

func zlibReply(t *testing.T, w http.ResponseWriter, body string) {
	t.Helper()
	w.Header().Set("Content-Type", "application/x-compress-zlib")
	b, err := zlibCompress([]byte(body))
	if err != nil {
		t.Fatal(err)
	}
	_, _ = w.Write(b)
}

func TestGLPIClientSendZlibRoundtrip(t *testing.T) {
	var gotAgentID, gotContentType string
	var gotBody []byte
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotAgentID = r.Header.Get("GLPI-Agent-ID")
		gotContentType = r.Header.Get("Content-Type")
		gotBody = readBody(t, r)
		zlibReply(t, w, `{"status":"ok","expiration":"24h"}`)
	}))
	defer srv.Close()

	c, err := NewGLPIClient(GLPIOptions{AgentID: "uuid-123", UserAgent: "GLPI-Agent_v2.17.0"})
	if err != nil {
		t.Fatal(err)
	}
	msg, err := c.Send(srv.URL, []byte(`{"action":"contact"}`))
	if err != nil {
		t.Fatal(err)
	}
	if gotAgentID != "uuid-123" {
		t.Errorf("GLPI-Agent-ID = %q", gotAgentID)
	}
	if gotContentType != "application/x-compress-zlib" {
		t.Errorf("Content-Type = %q", gotContentType)
	}
	if string(gotBody) != `{"action":"contact"}` {
		t.Errorf("server saw body %q", gotBody)
	}
	if msg.Status() != "ok" || msg.Expiration() != 24*3600 {
		t.Errorf("answer = status %q exp %d", msg.Status(), msg.Expiration())
	}
}

// TestGLPIClientUncompressed checks the no-compression path (plain JSON both ways).
func TestGLPIClientUncompressed(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Content-Type") != "application/json" {
			t.Errorf("Content-Type = %q, want application/json", r.Header.Get("Content-Type"))
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"status":"ok","expiration":"1h"}`))
	}))
	defer srv.Close()

	c, _ := NewGLPIClient(GLPIOptions{AgentID: "uuid", NoCompression: true})
	msg, err := c.Send(srv.URL, []byte(`{"action":"contact"}`))
	if err != nil {
		t.Fatal(err)
	}
	if msg.Status() != "ok" {
		t.Errorf("status = %q", msg.Status())
	}
}

// TestGLPIClientErrorStatus checks a status:"error" answer surfaces as an error
// with the server message.
func TestGLPIClientErrorStatus(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"status":"error","message":"bad agent id"}`))
	}))
	defer srv.Close()

	c, _ := NewGLPIClient(GLPIOptions{AgentID: "uuid"})
	_, err := c.Send(srv.URL, []byte(`{}`))
	if err == nil {
		t.Fatal("expected an error for status error")
	}
	if !bytes.Contains([]byte(err.Error()), []byte("bad agent id")) {
		t.Errorf("error = %v, want the server message", err)
	}
}

// TestGLPIClientBasicAuth checks credentials are sent.
func TestGLPIClientBasicAuth(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		u, p, ok := r.BasicAuth()
		if !ok || u != "agent" || p != "secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		zlibReply(t, w, `{"status":"ok","expiration":"24h"}`)
	}))
	defer srv.Close()

	c, _ := NewGLPIClient(GLPIOptions{AgentID: "uuid", User: "agent", Password: "secret"})
	if _, err := c.Send(srv.URL, []byte(`{}`)); err != nil {
		t.Fatalf("basic auth send failed: %v", err)
	}
}

func TestNewGLPIClientRequiresAgentID(t *testing.T) {
	if _, err := NewGLPIClient(GLPIOptions{}); err == nil {
		t.Error("expected an error without an agent id")
	}
}
