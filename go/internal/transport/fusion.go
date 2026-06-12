// SPDX-License-Identifier: GPL-2.0-only

package transport

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/cookiejar"
	"net/url"
	"sort"
	"strings"
	"time"
)

// FusionClient talks the legacy GLPI "Fusion" plugin protocol used by the
// Collect and Deploy tasks (HTTP/Client/Fusion.pm): requests carry an `action`
// plus arbitrary args encoded in the query string (GET) or the form body (POST),
// and the server replies with a JSON object. A cookie jar is kept across calls
// for the server session / CSRF handling.
type FusionClient struct {
	http      *http.Client
	userAgent string
	user      string
	password  string
}

// NewFusionClient builds a Fusion client reusing the shared TLS / proxy / auth
// configuration of the GLPI client.
func NewFusionClient(opts GLPIOptions) (*FusionClient, error) {
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
		tr.Proxy = http.ProxyFromEnvironment
	}

	timeout := opts.Timeout
	if timeout == 0 {
		timeout = 180 * time.Second
	}
	jar, _ := cookiejar.New(nil)

	return &FusionClient{
		http:      &http.Client{Transport: tr, Timeout: timeout, Jar: jar},
		userAgent: opts.UserAgent,
		user:      opts.User,
		password:  opts.Password,
	}, nil
}

// Send issues a Fusion request and returns the decoded JSON object. method is
// "GET" or "POST" (anything else is treated as GET, as upstream does). A nil map
// with a nil error means the server returned an empty/non-JSON body (upstream
// returns undef), which the caller treats as "nothing to do".
func (c *FusionClient) Send(rawURL, method string, args map[string]any) (map[string]any, error) {
	if method != "POST" {
		method = "GET"
	}
	params := encodeFusionArgs(args, method)

	var req *http.Request
	var err error
	if method == "GET" {
		req, err = http.NewRequest("GET", rawURL+"?"+params, nil)
	} else {
		// POST: action/uuid/method go on the query string, the args in the body.
		q := "action=" + url.QueryEscape(fusionStr(args["action"]))
		if uuid := fusionStr(args["uuid"]); uuid != "" {
			q += "&uuid=" + url.QueryEscape(uuid)
		}
		q += "&method=POST"
		req, err = http.NewRequest("POST", rawURL+"?"+q, strings.NewReader(params))
		if err == nil {
			req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
			req.Header.Set("Referer", rawURL)
		}
	}
	if err != nil {
		return nil, err
	}
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
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("server returned %s", resp.Status)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	if len(strings.TrimSpace(string(body))) == 0 {
		return nil, nil
	}
	var answer map[string]any
	if err := json.Unmarshal(body, &answer); err != nil {
		// A non-JSON body is treated as "no answer" (upstream returns undef).
		return nil, nil
	}
	return answer, nil
}

// encodeFusionArgs serialises the args the way Fusion.pm does: action first,
// then `k=v`, `k[]=v` for slices and `k[sub]=v` for maps; empty non-action
// values are skipped. method drives the GET length truncation of values.
func encodeFusionArgs(args map[string]any, method string) string {
	parts := []string{"action=" + url.QueryEscape(fusionStr(args["action"]))}

	keys := make([]string, 0, len(args))
	for k := range args {
		if k != "action" {
			keys = append(keys, k)
		}
	}
	sort.Strings(keys)

	for _, k := range keys {
		switch val := args[k].(type) {
		case []string:
			for _, e := range val {
				parts = append(parts, k+"[]="+prepareFusionVal(e, method))
			}
		case []any:
			for _, e := range val {
				parts = append(parts, k+"[]="+prepareFusionVal(fusionStr(e), method))
			}
		case map[string]string:
			for sk, sv := range val {
				parts = append(parts, k+"["+sk+"]="+prepareFusionVal(sv, method))
			}
		default:
			s := fusionStr(val)
			if s != "" {
				parts = append(parts, k+"="+prepareFusionVal(s, method))
			}
		}
	}
	return strings.Join(parts, "&")
}

// prepareFusionVal URL-encodes a value, truncating over-long GET values
// (Fusion.pm _prepareVal: keep escaped length under ~1500).
func prepareFusionVal(val, method string) string {
	if method == "GET" {
		for len(url.QueryEscape(val)) > 1500 && len(val) > 5 {
			val = "…" + val[5:]
		}
	}
	return url.QueryEscape(val)
}

// fusionStr renders a scalar arg value as a string.
func fusionStr(v any) string {
	switch t := v.(type) {
	case nil:
		return ""
	case string:
		return t
	case int:
		return fmt.Sprintf("%d", t)
	case int64:
		return fmt.Sprintf("%d", t)
	case float64:
		// Whole numbers without a decimal point.
		if t == float64(int64(t)) {
			return fmt.Sprintf("%d", int64(t))
		}
		return fmt.Sprintf("%v", t)
	case bool:
		if t {
			return "1"
		}
		return "0"
	default:
		return fmt.Sprintf("%v", t)
	}
}
