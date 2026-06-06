// SPDX-License-Identifier: GPL-2.0-only

// Package remote collects inventory from remote hosts over SSH (and, later,
// WinRM).
//
// Derived from the upstream Perl modules lib/GLPI/Agent/Task/RemoteInventory.pm
// and lib/GLPI/Agent/Task/RemoteInventory/Remote/Ssh.pm: commands are run as
// "LANG=C <command>", OSName comes from `uname -s` (with sunos->solaris and
// hp-ux->hpux), the hostname from `hostname`/`hostname -f`, file contents via
// `cat`, and the host-key policy from the stricthostkeychecking option. The
// transport uses golang.org/x/crypto/ssh instead of the system ssh binary or
// Net::SSH2.
package remote

import (
	"fmt"
	"net"
	"os"
	"strconv"
	"strings"
	"time"

	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

// Config describes an SSH connection, mirroring the options bin/glpi-remote and
// Remote/Ssh.pm understand.
type Config struct {
	Host            string
	Port            int // defaults to 22
	User            string
	Password        string
	IdentityFile    string // private key file (PEM)
	Timeout         time.Duration
	HostKeyChecking string // "strict" | "accept-new" (tofu, default) | "no"
	KnownHostsFile  string // defaults to ~/.ssh/known_hosts
}

// SSHClient is a connected SSH session factory.
type SSHClient struct {
	client *ssh.Client
}

// Dial connects and authenticates, mirroring the _connect logic of Remote/Ssh.pm
// (password authentication, then a private-key identity).
func Dial(cfg Config) (*SSHClient, error) {
	port := cfg.Port
	if port == 0 {
		port = 22
	}
	timeout := cfg.Timeout
	if timeout == 0 {
		timeout = 10 * time.Second // bin/glpi-remote default
	}

	var auths []ssh.AuthMethod
	if cfg.Password != "" {
		auths = append(auths, ssh.Password(cfg.Password))
	}
	if cfg.IdentityFile != "" {
		signer, err := loadIdentity(cfg.IdentityFile)
		if err != nil {
			return nil, err
		}
		auths = append(auths, ssh.PublicKeys(signer))
	}
	if len(auths) == 0 {
		return nil, fmt.Errorf("no SSH authentication method: provide a password or an identity file")
	}

	hostKeyCallback, err := hostKeyCallback(cfg)
	if err != nil {
		return nil, err
	}

	clientCfg := &ssh.ClientConfig{
		User:            cfg.User,
		Auth:            auths,
		HostKeyCallback: hostKeyCallback,
		Timeout:         timeout,
	}

	addr := net.JoinHostPort(cfg.Host, strconv.Itoa(port))
	client, err := ssh.Dial("tcp", addr, clientCfg)
	if err != nil {
		return nil, fmt.Errorf("can't reach %s for ssh remoteinventory: %w", addr, err)
	}
	return &SSHClient{client: client}, nil
}

// Close terminates the connection.
func (c *SSHClient) Close() error { return c.client.Close() }

// Run executes a command remotely as "LANG=C <command>" and returns its trimmed
// standard output, mirroring getRemoteFileHandle(command => ...).
func (c *SSHClient) Run(command string) (string, error) {
	session, err := c.client.NewSession()
	if err != nil {
		return "", err
	}
	defer session.Close()
	out, err := session.Output("LANG=C " + command)
	if err != nil {
		return "", err
	}
	return strings.TrimRight(string(out), "\n"), nil
}

// firstLine runs a command and returns its first output line, mirroring
// getRemoteFirstLine.
func (c *SSHClient) firstLine(command string) (string, error) {
	out, err := c.Run(command)
	if err != nil {
		return "", err
	}
	if i := strings.IndexByte(out, '\n'); i >= 0 {
		return out[:i], nil
	}
	return out, nil
}

// ReadFile returns the content of a remote file via `cat`, mirroring the file
// branch of getRemoteFileHandle.
func (c *SSHClient) ReadFile(path string) (string, error) {
	return c.Run(fmt.Sprintf("cat '%s'", path))
}

// OSName mirrors OSName(): lowercased `uname -s`, normalising SunOS and HP-UX.
func (c *SSHClient) OSName() (string, error) {
	out, err := c.firstLine("uname -s")
	if err != nil {
		return "", err
	}
	osname := strings.ToLower(out)
	switch osname {
	case "sunos":
		return "solaris", nil
	case "hp-ux":
		return "hpux", nil
	}
	return osname, nil
}

// Hostname mirrors getRemoteHostname(): `hostname`, falling back to the config
// host.
func (c *SSHClient) Hostname(fallback string) string {
	if h, err := c.firstLine("hostname"); err == nil && h != "" {
		return h
	}
	return fallback
}

// FQDN mirrors getRemoteFQDN(): `hostname -f`.
func (c *SSHClient) FQDN() string {
	if f, err := c.firstLine("hostname -f"); err == nil {
		return f
	}
	return ""
}

// CanRun mirrors remoteCanRun(): `test -x` for an absolute path, otherwise
// `which`.
func (c *SSHClient) CanRun(binary string) bool {
	var command string
	if strings.HasPrefix(binary, "/") {
		command = fmt.Sprintf("test -x '%s'", binary)
	} else {
		command = fmt.Sprintf("which %s >/dev/null", binary)
	}
	_, err := c.Run(command)
	return err == nil
}

func loadIdentity(path string) (ssh.Signer, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("can't read identity file %s: %w", path, err)
	}
	signer, err := ssh.ParsePrivateKey(data)
	if err != nil {
		return nil, fmt.Errorf("can't parse identity file %s: %w", path, err)
	}
	return signer, nil
}

// hostKeyCallback maps the stricthostkeychecking option onto an
// ssh.HostKeyCallback, mirroring the policy selection in Remote/Ssh.pm
// (tofu default, strict, advisory/no).
func hostKeyCallback(cfg Config) (ssh.HostKeyCallback, error) {
	switch strings.ToLower(cfg.HostKeyChecking) {
	case "no", "advisory", "off", "false":
		return ssh.InsecureIgnoreHostKey(), nil //nolint:gosec // explicit opt-out
	case "strict", "yes":
		return knownHosts(cfg, false)
	default: // "accept-new", "tofu", "" -> trust on first use
		return knownHosts(cfg, true)
	}
}

// knownHosts builds a callback backed by a known_hosts file. When acceptNew is
// set, an unknown host key is trusted and appended (trust on first use);
// a key that conflicts with a stored one is always rejected.
func knownHosts(cfg Config, acceptNew bool) (ssh.HostKeyCallback, error) {
	path := cfg.KnownHostsFile
	if path == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return nil, err
		}
		path = home + "/.ssh/known_hosts"
	}
	// Ensure the file exists so knownhosts.New does not fail, mirroring the Perl
	// code that creates an empty known_hosts when missing.
	if _, err := os.Stat(path); os.IsNotExist(err) {
		_ = os.MkdirAll(dir(path), 0o700)
		if f, err := os.OpenFile(path, os.O_CREATE, 0o600); err == nil {
			_ = f.Close()
		}
	}
	base, err := knownhosts.New(path)
	if err != nil {
		return nil, err
	}
	if !acceptNew {
		return base, nil
	}
	return func(hostname string, remote net.Addr, key ssh.PublicKey) error {
		err := base(hostname, remote, key)
		if err == nil {
			return nil
		}
		var keyErr *knownhosts.KeyError
		if asKeyError(err, &keyErr) && len(keyErr.Want) > 0 {
			// Host present with a different key -> never auto-trust.
			return err
		}
		// Unknown host -> trust on first use and persist.
		return appendKnownHost(path, hostname, key)
	}, nil
}
