// SPDX-License-Identifier: GPL-2.0-only

// Package transport holds the GLPI HTTP client. The inject path here is derived
// from bin/glpi-injector (the standalone pusher); the shared inventory client
// from lib/GLPI/Agent/HTTP/Client/** is added later in Phase 1.
package transport

import (
	"bytes"
	"compress/zlib"
	"crypto/tls"
	"fmt"
	"io"
	"net/http"
	"strings"
)

// InjectRequest describes one content push, mirroring the per-file logic of
// sendContent() in bin/glpi-injector.
type InjectRequest struct {
	URL           string
	Content       []byte
	UserAgent     string
	AgentID       string // GLPI-Agent-ID header; set for JSON content
	NoCompression bool
	NoSSLCheck    bool
}

// Inject POSTs the content to the server and reports success, mirroring the
// header and compression handling of bin/glpi-injector::sendContent. The
// OAuth-token and pending-proxy paths of the Perl script are not implemented
// yet (Phase 1).
func Inject(req InjectRequest) error {
	body := req.Content
	contentType := jsonOrXMLContentType(req.AgentID, req.Content)
	if !req.NoCompression {
		var buf bytes.Buffer
		zw := zlib.NewWriter(&buf)
		if _, err := zw.Write(req.Content); err != nil {
			return fmt.Errorf("compressing content: %w", err)
		}
		if err := zw.Close(); err != nil {
			return fmt.Errorf("compressing content: %w", err)
		}
		body = buf.Bytes()
		contentType = "Application/x-compress-zlib"
	}

	httpReq, err := http.NewRequest(http.MethodPost, req.URL, bytes.NewReader(body))
	if err != nil {
		return err
	}
	httpReq.Header.Set("Pragma", "no-cache")
	httpReq.Header.Set("Content-Type", contentType)
	if req.UserAgent != "" {
		httpReq.Header.Set("User-Agent", req.UserAgent)
	}
	if req.AgentID != "" {
		httpReq.Header.Set("GLPI-Agent-ID", req.AgentID)
	}

	client := &http.Client{}
	if req.NoSSLCheck {
		client.Transport = &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true}, //nolint:gosec // explicit --no-ssl-check
		}
	}

	resp, err := client.Do(httpReq)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	respBody, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("%s: %s", resp.Status, strings.TrimSpace(string(respBody)))
	}
	if err := serverError(respBody); err != nil {
		return err
	}
	return nil
}

// jsonOrXMLContentType picks the uncompressed content type the way
// bin/glpi-injector does: JSON when an agent id is present, otherwise XML.
func jsonOrXMLContentType(agentID string, content []byte) string {
	if agentID != "" || bytes.HasPrefix(bytes.TrimSpace(content), []byte("{")) {
		return "Application/json"
	}
	return "Application/xml"
}

// serverError inspects a JSON answer for a `status:"error"` reply, mirroring the
// error extraction in sendContent.
func serverError(body []byte) error {
	trimmed := bytes.TrimSpace(body)
	if !bytes.HasPrefix(trimmed, []byte("{")) {
		return nil
	}
	// Minimal check without a full JSON decode dependency on the answer schema:
	// the Perl side treats status "error" as a failure and surfaces "message".
	if bytes.Contains(trimmed, []byte(`"status"`)) && bytes.Contains(trimmed, []byte(`"error"`)) {
		return fmt.Errorf("server returned error status: %s", string(trimmed))
	}
	return nil
}
