// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"crypto/ed25519"
	"crypto/rand"
	"net"
	"strings"
	"testing"
	"time"

	"golang.org/x/crypto/ssh"
)

// cannedCommands are the responses the in-process SSH server returns, keyed by
// the command the client runs (without the "LANG=C " prefix the client adds).
var cannedCommands = map[string]string{
	"hostname":            "remote-box",
	"hostname -f":         "remote-box.example.com",
	"uname -s":            "Linux",
	"uname -r":            "5.15.0-generic",
	"uname -m":            "x86_64",
	"which sh >/dev/null": "",
}

// TestSSHCollectInventory runs the SSH client against a real in-process SSH
// server, exercising the connect/auth/exec path and the Perl-derived field
// mapping end to end.
func TestSSHCollectInventory(t *testing.T) {
	host, port := startTestSSHServer(t)

	client, err := Dial(Config{
		Host:            host,
		Port:            port,
		User:            "tester",
		Password:        "secret",
		Timeout:         5 * time.Second,
		HostKeyChecking: "no",
	})
	if err != nil {
		t.Fatalf("Dial: %v", err)
	}
	defer client.Close()

	if got, _ := client.OSName(); got != "linux" {
		t.Errorf("OSName = %q, want linux", got)
	}
	if got := client.Hostname("fallback"); got != "remote-box" {
		t.Errorf("Hostname = %q, want remote-box", got)
	}
	if !client.CanRun("sh") {
		t.Error("CanRun(sh) = false, want true")
	}

	inv, err := client.CollectInventory("Computer", "lab", host)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(inv.DeviceID, "remote-box-") {
		t.Errorf("deviceid = %q, want it to start with remote-box-", inv.DeviceID)
	}
	hw := inv.Content["HARDWARE"].(map[string]any)
	if hw["NAME"] != "remote-box" || hw["ARCH"] != "x86_64" {
		t.Errorf("HARDWARE = %v, want NAME=remote-box ARCH=x86_64", hw)
	}
	os := inv.Content["OPERATINGSYSTEM"].(map[string]any)
	if os["KERNEL_NAME"] != "linux" || os["KERNEL_VERSION"] != "5.15.0-generic" {
		t.Errorf("OPERATINGSYSTEM = %v", os)
	}
	if os["FQDN"] != "remote-box.example.com" {
		t.Errorf("FQDN = %v, want remote-box.example.com", os["FQDN"])
	}
}

// startTestSSHServer spins up a minimal SSH server that accepts password auth
// and answers exec requests from cannedCommands. It returns the host and port.
func startTestSSHServer(t *testing.T) (string, int) {
	t.Helper()

	_, priv, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	signer, err := ssh.NewSignerFromSigner(priv)
	if err != nil {
		t.Fatal(err)
	}

	cfg := &ssh.ServerConfig{
		PasswordCallback: func(c ssh.ConnMetadata, pass []byte) (*ssh.Permissions, error) {
			if c.User() == "tester" && string(pass) == "secret" {
				return nil, nil
			}
			return nil, errAuth
		},
	}
	cfg.AddHostKey(signer)

	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = ln.Close() })

	go func() {
		for {
			conn, err := ln.Accept()
			if err != nil {
				return
			}
			go serveConn(conn, cfg)
		}
	}()

	addr := ln.Addr().(*net.TCPAddr)
	return "127.0.0.1", addr.Port
}

func serveConn(conn net.Conn, cfg *ssh.ServerConfig) {
	sconn, chans, reqs, err := ssh.NewServerConn(conn, cfg)
	if err != nil {
		return
	}
	defer sconn.Close()
	go ssh.DiscardRequests(reqs)

	for newChan := range chans {
		if newChan.ChannelType() != "session" {
			_ = newChan.Reject(ssh.UnknownChannelType, "only session")
			continue
		}
		ch, chReqs, err := newChan.Accept()
		if err != nil {
			continue
		}
		go handleSession(ch, chReqs)
	}
}

func handleSession(ch ssh.Channel, reqs <-chan *ssh.Request) {
	for req := range reqs {
		if req.Type != "exec" {
			if req.WantReply {
				_ = req.Reply(false, nil)
			}
			continue
		}
		var payload struct{ Command string }
		_ = ssh.Unmarshal(req.Payload, &payload)
		command := strings.TrimPrefix(payload.Command, "LANG=C ")

		if req.WantReply {
			_ = req.Reply(true, nil)
		}
		out, known := cannedCommands[command]
		if out != "" {
			_, _ = ch.Write([]byte(out + "\n"))
		}
		status := uint32(0)
		if !known {
			status = 1
		}
		_, _ = ch.SendRequest("exit-status", false, ssh.Marshal(struct{ Status uint32 }{status}))
		_ = ch.Close()
	}
}

var errAuth = &authError{}

type authError struct{}

func (*authError) Error() string { return "authentication failed" }
