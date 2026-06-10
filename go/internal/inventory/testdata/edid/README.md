<!-- SPDX-License-Identifier: GPL-2.0-only -->

# EDID fixtures

Real 128-byte EDID blobs **vendored verbatim from the upstream GLPI Agent** test
suite (`resources/generic/edid/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project). `edid_fixtures_test.go` parses each with the
Go `BuildMonitor` and asserts the values
`t/tasks/inventory/generic/screen.t` expects — manufacturer (resolved via the
embedded `edid.ids` vendor database), caption (monitor-name descriptor),
description (week/year, including the year-only `week == 255` case) and, for the
simple numeric/plain serials, the serial.

Known divergence: for monitors whose upstream `SERIAL` is a Parse::EDID
*combined* value (those that also carry an `ALTSERIAL`), Go emits the raw
descriptor serial instead of replicating that combination, so the exact serial
is not pinned for those blobs. A couple of blobs also carry a `"0"` serial
descriptor that upstream ignores.
