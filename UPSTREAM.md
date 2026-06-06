<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Upstream sync point

This project is a Rust rewrite of the upstream **Perl** GLPI agent. To be able to
adopt upstream fixes and features predictably, we pin the exact upstream commit
this codebase is in feature parity with, and bump it deliberately.

The module-by-module mapping of upstream features to their Rust counterparts (and
what is still missing) lives in [docs/UPSTREAM-MAPPING.md](docs/UPSTREAM-MAPPING.md).

- **Upstream:** <https://github.com/glpi-project/glpi-agent>
- **Synced to:** commit
  [`24fec367fffadbfe373b57c61085f9fe20ce51f5`](https://github.com/glpi-project/glpi-agent/commit/24fec367fffadbfe373b57c61085f9fe20ce51f5)
  (the GLPI `1.17` line, including commits made after the `1.17` tag)
- **Last reviewed:** 2026-06-06

## Version mapping

The Rust version is kept **in step with the upstream GLPI agent version**, with
the major bumped to 2 to separate it from the Perl 1.x line:

> GLPI `1.MINOR[.PATCH]`  →  Rust `2.MINOR[.PATCH]`

So while upstream's latest release is **1.17**, this rewrite is **2.17.x**. When
the pinned commit is moved onto a newer upstream release (say `1.18`), bump the
Rust version to `2.18.0` in the **same** PR. The version lives in one place,
`[workspace.package] version` in [Cargo.toml](Cargo.toml); everything the agent
reports (`--version`, the `versionclient` it sends GLPI, the HTTP user-agent, and
the package versions) derives from it via `CARGO_PKG_VERSION`.

This file is the source of truth for *which upstream state we are level with*;
the version number above encodes *which upstream release line* that is.

## Why pin a commit

The migration was done against a specific upstream state (GLPI Agent 1.17, the
parity target named in [the migration plan](glpi-agent-rust-migration-plan.md)).
Recording the exact commit lets us diff any later upstream state against a known
baseline, so an upstream bug fix or new vendor MIB can be located and ported
without re-reviewing the entire history.

## How to adopt upstream changes

1. Find the new upstream ref you want to move to (a release tag is usually best):

   ```sh
   git ls-remote --tags https://github.com/glpi-project/glpi-agent
   ```

2. Review what changed since the pinned commit:

   <https://github.com/glpi-project/glpi-agent/compare/24fec367fffadbfe373b57c61085f9fe20ce51f5...TARGET>

3. Port the relevant changes crate by crate. Migrate the upstream test/fixture
   alongside each change in the same step — a change is "done" only when its
   migrated test passes (see the migration plan, §7 and §13).

4. Reconcile the module mapping in [docs/UPSTREAM-MAPPING.md](docs/UPSTREAM-MAPPING.md):
   a new upstream module/section with no row there is a freshly-introduced gap.

5. In the **same** PR, update the **Synced to** commit and the **Last reviewed**
   date above, bump the Rust version per the mapping when crossing a release, and
   add a row to the history table below.

## Sync history

| Date       | Upstream ref           | Commit    | Rust version | Notes                                          |
| ---------- | ---------------------- | --------- | ------------ | ---------------------------------------------- |
| 2026-06-06 | `1.17` line (+ later)  | `24fec36` | `2.17.0`     | Parity baseline; pinned past the `1.17` tag.   |
