<!-- SPDX-License-Identifier: GPL-2.0-only -->

# TeamViewer fixtures

Real `teamviewer --info` output **vendored verbatim from the upstream GLPI
Agent** test suite (`resources/generic/teamviewer/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0).
`antivirus_fixtures_test.go` pins `ParseTeamViewerInfo` against the upstream
`t/.../remote_mgmt/teamviewer.t` expectation (`15.65.4-DEB` → ID `552`).
