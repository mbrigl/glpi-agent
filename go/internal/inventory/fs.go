// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"os"
	"path/filepath"
)

// FS abstracts the filesystem access the file-based (sysfs) collectors need, so
// they can run against a remote host (RemoteInventory) as well as the local one
// — the Go counterpart of the upstream GLPI::Agent::Tools::remote seam for file
// reads.
type FS interface {
	ReadFile(path string) ([]byte, error)
	Glob(pattern string) ([]string, error)
}

// osFS is the default, local filesystem.
type osFS struct{}

func (osFS) ReadFile(path string) ([]byte, error)  { return os.ReadFile(path) }
func (osFS) Glob(pattern string) ([]string, error) { return filepath.Glob(pattern) }

// invFS is the filesystem the file-based collectors read through. It defaults to
// the local one; CollectFileSectionsFS swaps in a remote FS for the duration of
// a remote inventory (inventory collection is single-threaded per call).
var invFS FS = osFS{}

// CollectFileSectionsFS runs the sysfs/file-based collectors against the given
// filesystem and returns the sections they produce (BATTERIES, USBDEVICES,
// STORAGES). It is used by RemoteInventory to read a remote host's sysfs over
// SSH; the root "/" makes the absolute sysfs paths resolve through fs.
func CollectFileSectionsFS(fs FS) Sections {
	old := invFS
	invFS = fs
	defer func() { invFS = old }()

	s := Sections{}
	if b := BuildBatteries("/"); len(b) > 0 {
		s["BATTERIES"] = b
	}
	if u := BuildUSB("/"); len(u) > 0 {
		s["USBDEVICES"] = u
	}
	if st := BuildStorages("/"); len(st) > 0 {
		s["STORAGES"] = st
	}
	return s
}
