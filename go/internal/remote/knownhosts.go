// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"errors"
	"os"
	"path/filepath"

	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

func dir(path string) string { return filepath.Dir(path) }

func asKeyError(err error, target **knownhosts.KeyError) bool {
	return errors.As(err, target)
}

// appendKnownHost appends a host key line to the known_hosts file, implementing
// the trust-on-first-use persistence used by the accept-new policy.
func appendKnownHost(path, hostname string, key ssh.PublicKey) error {
	line := knownhosts.Line([]string{knownhosts.Normalize(hostname)}, key)
	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = f.WriteString(line + "\n")
	return err
}
