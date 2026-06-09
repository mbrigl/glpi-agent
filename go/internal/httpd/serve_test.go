// SPDX-License-Identifier: GPL-2.0-only

package httpd

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"io"
	"math/big"
	"net"
	"net/http"
	"testing"
	"time"
)

// selfSignedTLS builds a minimal self-signed certificate TLS config for the
// HTTPS test.
func selfSignedTLS(t *testing.T) *tls.Config {
	t.Helper()
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	tmpl := &x509.Certificate{
		SerialNumber: big.NewInt(1),
		Subject:      pkix.Name{CommonName: "localhost"},
		NotBefore:    time.Now().Add(-time.Hour),
		NotAfter:     time.Now().Add(time.Hour),
		IPAddresses:  []net.IP{net.ParseIP("127.0.0.1")},
	}
	der, err := x509.CreateCertificate(rand.Reader, tmpl, tmpl, &key.PublicKey, key)
	if err != nil {
		t.Fatal(err)
	}
	return &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{der}, PrivateKey: key}}}
}

// TestServeTLS checks the control server serves HTTPS when given a TLS config.
func TestServeTLS(t *testing.T) {
	s := testServer(t, &fakeAgent{status: "waiting"}, nil)
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go func() { _ = s.Serve(ctx, ln, selfSignedTLS(t)) }()

	client := &http.Client{Transport: &http.Transport{TLSClientConfig: &tls.Config{InsecureSkipVerify: true}}}
	resp, err := client.Get("https://" + ln.Addr().String() + "/status")
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(resp.Body)
	resp.Body.Close()
	if string(body) != "status: waiting" {
		t.Errorf("https /status body = %q", body)
	}
}

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
	go func() { _ = s.Serve(ctx, ln, nil); close(done) }()

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
