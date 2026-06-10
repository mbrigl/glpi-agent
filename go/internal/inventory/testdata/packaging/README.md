<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Packaging fixtures

The `rpm` file is the real `rpm -qa --qf '…'` capture **vendored verbatim from
the upstream GLPI Agent** test suite (`resources/linux/packaging/rpm` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project) — the tab-separated
name/arch/version/installdate/size/vendor/summary/group columns the Go
`ParseRPMQA` consumes. `rpm_fixtures_test.go` pins the package count and the
first package's fields.

The upstream `dpkg` fixture is **not** vendored: it is `dpkg-query -W` tab output,
whereas the Go deb collector parses the `/var/lib/dpkg/status` stanza file
directly (`ParseDpkgStatus`), a different — and external-command-free —
implementation, so the two are not interchangeable.
