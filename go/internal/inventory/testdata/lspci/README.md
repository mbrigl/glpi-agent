<!-- SPDX-License-Identifier: GPL-2.0-only -->

# lspci fixtures

Real `lspci -nn` capture files **vendored verbatim from the upstream GLPI
Agent** test suite (`resources/generic/lspci/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project). They are used by `lspci_fixtures_test.go` to
exercise the Go `ParseLspci` / `BuildControllers` / `BuildVideos` /
`BuildSounds` parsers against the same real-world inputs the Perl agent is
tested on.

Replaying these immediately surfaced a real parser bug: the header regex was
anchored right after the optional `(rev X)`, so it silently dropped every device
line carrying a trailing annotation such as `(prog-if 00 [VGA controller])`
(e.g. only 9 of 24 devices parsed on `dell-xt2`, losing the integrated GPU). The
regex now tolerates trailing content.
