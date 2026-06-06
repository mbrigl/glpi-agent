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

// TestInjectCompressed verifies the compressed POST path mirrors
// bin/glpi-injector: Content-Type x-compress-zlib, the GLPI-Agent-ID header for
// JSON content, and a body that inflates back to the original.
func TestInjectCompressed(t *testing.T) {
	original := []byte(`{"deviceid":"host-1","action":"inventory"}`)

	var gotType, gotAgentID, gotUA string
	var gotBody []byte
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotType = r.Header.Get("Content-Type")
		gotAgentID = r.Header.Get("GLPI-Agent-ID")
		gotUA = r.Header.Get("User-Agent")
		gotBody, _ = io.ReadAll(r.Body)
		_, _ = w.Write([]byte(`{"status":"ok"}`))
	}))
	defer srv.Close()

	err := Inject(InjectRequest{
		URL:       srv.URL,
		Content:   original,
		UserAgent: "GLPI-Injector",
		AgentID:   "11111111-2222-4333-8444-555555555555",
	})
	if err != nil {
		t.Fatalf("Inject returned error: %v", err)
	}

	if gotType != "Application/x-compress-zlib" {
		t.Errorf("Content-Type = %q, want Application/x-compress-zlib", gotType)
	}
	if gotAgentID != "11111111-2222-4333-8444-555555555555" {
		t.Errorf("GLPI-Agent-ID = %q", gotAgentID)
	}
	if gotUA != "GLPI-Injector" {
		t.Errorf("User-Agent = %q", gotUA)
	}

	zr, err := zlib.NewReader(bytes.NewReader(gotBody))
	if err != nil {
		t.Fatalf("body is not zlib-compressed: %v", err)
	}
	defer zr.Close()
	inflated, _ := io.ReadAll(zr)
	if !bytes.Equal(inflated, original) {
		t.Errorf("inflated body = %q, want %q", inflated, original)
	}
}

// TestInjectServerError checks that a JSON error reply is surfaced as a failure,
// mirroring the status:"error" handling in sendContent.
func TestInjectServerError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`{"status":"error","message":"bad import"}`))
	}))
	defer srv.Close()

	err := Inject(InjectRequest{URL: srv.URL, Content: []byte("{}"), NoCompression: true})
	if err == nil {
		t.Fatal("expected an error for status:error reply, got nil")
	}
}
