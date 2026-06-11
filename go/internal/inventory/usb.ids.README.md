# usb.ids database

`usb.ids` is a verbatim copy of the file shipped in the upstream GLPI Agent
(Perl) `share/usb.ids` (version 2025.12.13). It is the public USB ID repository
maintained at <http://www.linux-usb.org/usb-ids.html>, distributed under the
GPL-2.0-or-later / BSD dual licence — compatible with this GPL-2.0 Go track.

It is parsed by `usbids.go` and embedded into the **Windows** build only
(`winusb_windows.go`, `//go:build windows`) to resolve USBDEVICES vendor/device
names (Win32/USB.pm). Non-Windows binaries do not embed it.

To refresh: copy `share/usb.ids` from the pinned upstream Perl checkout.
