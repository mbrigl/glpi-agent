// SPDX-License-Identifier: GPL-2.0-only

package httpd

import (
	"context"
	"io"
	"net"
	"net/http"
	"testing"
	"time"
)

// TestServeEndToEnd starts the control server on an ephemeral port, hits /status
// and the trusted /now over a real socket, and checks it shuts down on cancel.
func TestServeEndToEnd(t *testing.T) {
	fa := &fakeAgent{status: "waiting"}
	s := testServer(t, fa, []string{"127.0.0.1"})

	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan struct{})
	go func() { _ = s.Serve(ctx, ln); close(done) }()

	baseURL := "http://" + ln.Addr().String()

	// /status over the real socket.
	resp, err := http.Get(baseURL + "/status")
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	if string(body) != "status: waiting" {
		t.Errorf("/status body = %q", body)
	}

	// /now from the trusted loopback triggers a run.
	resp, err = http.Get(baseURL + "/now")
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != 200 || fa.runNowed != 1 {
		t.Errorf("/now status=%d runNowed=%d", resp.StatusCode, fa.runNowed)
	}

	// Cancelling the context shuts the server down.
	cancel()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("Serve did not return after cancel")
	}
}
