// SPDX-License-Identifier: GPL-2.0-only

package transport

import (
	"bytes"
	"compress/gzip"
	"compress/zlib"
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/protocol"
)

// GLPIOptions configures the GLPI HTTP client, mirroring the parameters of
// lib/GLPI/Agent/HTTP/Client.pm (TLS, proxy, basic auth, timeout, compression).
type GLPIOptions struct {
	AgentID   string // GLPI-Agent-ID (UUID); mandatory, the server identifies the agent by it
	UserAgent string
	Timeout   time.Duration // default 180s (Client.pm)

	NoCompression bool // send/accept uncompressed JSON instead of zlib

	// TLS.
	NoSSLCheck  bool
	CACertFile  string
	CACertDir   string
	SSLCertFile string // client certificate (mutual TLS)
	SSLKeyFile  string

	// Auth / proxy.
	User     string
	Password string
	Proxy    string // "" = use environment proxy; "none" = no proxy; otherwise a proxy URL
}

// GLPIClient sends GLPI protocol messages to a server, derived from
// HTTP/Client.pm + HTTP/Client/GLPI.pm. It POSTs a (optionally zlib-compressed)
// JSON message and returns the parsed server answer.
type GLPIClient struct {
	http        *http.Client
	agentID     string
	userAgent   string
	user        string
	password    string
	compression string // "zlib" or "" (none)
}

// NewGLPIClient builds a client with the full TLS / proxy / auth configuration.
func NewGLPIClient(opts GLPIOptions) (*GLPIClient, error) {
	if opts.AgentID == "" {
		return nil, fmt.Errorf("a GLPI-Agent-ID is required")
	}

	tlsConfig, err := buildTLSConfig(opts)
	if err != nil {
		return nil, err
	}
	tr := &http.Transport{TLSClientConfig: tlsConfig}

	switch {
	case opts.Proxy == "none":
		tr.Proxy = nil
	case opts.Proxy != "":
		u, err := url.Parse(opts.Proxy)
		if err != nil {
			return nil, fmt.Errorf("invalid proxy %q: %w", opts.Proxy, err)
		}
		tr.Proxy = http.ProxyURL(u)
	default:
		tr.Proxy = http.ProxyFromEnvironment // env_proxy
	}

	timeout := opts.Timeout
	if timeout == 0 {
		timeout = 180 * time.Second
	}

	compression := "zlib"
	if opts.NoCompression {
		compression = ""
	}

	return &GLPIClient{
		http:        &http.Client{Transport: tr, Timeout: timeout},
		agentID:     opts.AgentID,
		userAgent:   opts.UserAgent,
		user:        opts.User,
		password:    opts.Password,
		compression: compression,
	}, nil
}

// Send POSTs a JSON message to the server and returns the parsed answer,
// mirroring HTTP/Client/GLPI.pm::send: compress, POST with the GLPI-Agent-ID
// header, decompress the reply by its content type, and parse it as a GLPI
// protocol message. A server answer with status "error" is returned together
// with an error carrying the server message.
func (c *GLPIClient) Send(serverURL string, message []byte) (*protocol.Message, error) {
	body := message
	contentType := "application/json"
	if c.compression == "zlib" {
		compressed, err := zlibCompress(message)
		if err != nil {
			return nil, fmt.Errorf("compressing message: %w", err)
		}
		body = compressed
		contentType = "application/x-compress-zlib"
	}

	req, err := http.NewRequest(http.MethodPost, serverURL, bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Pragma", "no-cache")
	req.Header.Set("Content-Type", contentType)
	req.Header.Set("GLPI-Agent-ID", c.agentID)
	if c.userAgent != "" {
		req.Header.Set("User-Agent", c.userAgent)
	}
	if c.user != "" {
		req.SetBasicAuth(c.user, c.password)
	}

	resp, err := c.http.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	raw, err := io.ReadAll(io.LimitReader(resp.Body, 32<<20))
	if err != nil {
		return nil, err
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("%s: %s", resp.Status, strings.TrimSpace(string(raw)))
	}
	if len(raw) == 0 {
		return nil, fmt.Errorf("server returned an empty answer")
	}

	content, err := uncompressByType(raw, resp.Header.Get("Content-Type"))
	if err != nil {
		return nil, err
	}
	msg, err := protocol.Parse(content)
	if err != nil {
		return nil, fmt.Errorf("invalid server answer: %w", err)
	}
	if msg.Status() == "error" {
		detail := msg.Message()
		if detail == "" {
			detail = string(content)
		}
		return msg, fmt.Errorf("server returned an error: %s", detail)
	}
	return msg, nil
}

// buildTLSConfig assembles the TLS configuration from the SSL options.
func buildTLSConfig(opts GLPIOptions) (*tls.Config, error) {
	cfg := &tls.Config{MinVersion: tls.VersionTLS12}

	if opts.NoSSLCheck {
		cfg.InsecureSkipVerify = true //nolint:gosec // explicit --no-ssl-check
		return cfg, nil
	}

	// Custom CA roots (file and/or directory) on top of the system store.
	if opts.CACertFile != "" || opts.CACertDir != "" {
		pool, err := x509.SystemCertPool()
		if err != nil || pool == nil {
			pool = x509.NewCertPool()
		}
		if opts.CACertFile != "" {
			if err := appendPEMFile(pool, opts.CACertFile); err != nil {
				return nil, err
			}
		}
		if opts.CACertDir != "" {
			entries, err := os.ReadDir(opts.CACertDir)
			if err != nil {
				return nil, fmt.Errorf("reading ca-cert-dir: %w", err)
			}
			for _, e := range entries {
				if e.IsDir() {
					continue
				}
				// Ignore non-certificate files silently, like the Perl CA scan.
				_ = appendPEMFile(pool, filepath.Join(opts.CACertDir, e.Name()))
			}
		}
		cfg.RootCAs = pool
	}

	// Client certificate (mutual TLS).
	if opts.SSLCertFile != "" || opts.SSLKeyFile != "" {
		if opts.SSLCertFile == "" || opts.SSLKeyFile == "" {
			return nil, fmt.Errorf("both ssl-cert-file and ssl-key-file are required for client certificates")
		}
		cert, err := tls.LoadX509KeyPair(opts.SSLCertFile, opts.SSLKeyFile)
		if err != nil {
			return nil, fmt.Errorf("loading client certificate: %w", err)
		}
		cfg.Certificates = []tls.Certificate{cert}
	}

	return cfg, nil
}

func appendPEMFile(pool *x509.CertPool, path string) error {
	pem, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("reading certificate %s: %w", path, err)
	}
	if !pool.AppendCertsFromPEM(pem) {
		return fmt.Errorf("no certificates found in %s", path)
	}
	return nil
}

// zlibCompress zlib-compresses data (Client.pm compress, zlib mode).
func zlibCompress(data []byte) ([]byte, error) {
	var buf bytes.Buffer
	zw := zlib.NewWriter(&buf)
	if _, err := zw.Write(data); err != nil {
		return nil, err
	}
	if err := zw.Close(); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// uncompressByType decodes a response body by its content type, mirroring
// Client.pm::uncompress: x-compress-zlib / x-compress-gzip are inflated, json /
// xml / empty types are returned as-is, and an otherwise JSON-looking body is
// accepted.
func uncompressByType(data []byte, contentType string) ([]byte, error) {
	t := strings.ToLower(contentType)
	if i := strings.IndexByte(t, ';'); i >= 0 {
		t = t[:i]
	}
	t = strings.TrimSpace(strings.TrimPrefix(t, "application/"))

	switch t {
	case "x-compress-zlib":
		return zlibDecompress(data)
	case "x-compress-gzip":
		return gzipDecompress(data)
	case "json", "xml":
		return data, nil
	}
	// Fall back to content sniffing for an unspecified type.
	if bytes.HasPrefix(bytes.TrimSpace(data), []byte("{")) {
		return data, nil
	}
	return nil, fmt.Errorf("unsupported answer content type %q", contentType)
}

func zlibDecompress(data []byte) ([]byte, error) {
	zr, err := zlib.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("inflating zlib answer: %w", err)
	}
	defer zr.Close()
	return io.ReadAll(zr)
}

func gzipDecompress(data []byte) ([]byte, error) {
	gr, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("inflating gzip answer: %w", err)
	}
	defer gr.Close()
	return io.ReadAll(gr)
}
