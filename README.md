[![Status](https://img.shields.io/github/actions/workflow/status/mbrigl/glpi-agent/ci.yml?label=Build)](https://github.com/mbrigl/glpi-agent/actions/workflows/ci.yml)
[![GitHub License](https://img.shields.io/github/license/mbrigl/glpi-agent?label=License)](https://opensource.org/license/gpl-2.0)


# GLPI Agent in Rust

A **Rust rewrite** of the [GLPI inventory agent](https://github.com/glpi-project/glpi-agent). The
upstream agent is written in Perl; this project re-implements it as a Cargo workspace of focused
crates while staying compatible with the GLPI inventory protocol. It starts at **v2.0.0** to
separate it from the Perl 1.x line.

> **Status: all nine tasks implemented; stabilization (Phase 10) under way.** The Cargo workspace and
> its member crates are in place; the cross-platform surface is implemented and tested, with the
> remaining work being platform-specific inventory (Windows / macOS / exotic Unix) and packaging.
>
> Phase status (see [AGENTS.md](AGENTS.md) for the per-crate breakdown):
>
> | Phase | Area | Status |
> | ----- | ---- | ------ |
> | 1 | Foundation — `glpi-core`, `glpi-transport` | ✅ complete |
> | 2–3 | NetDiscovery + NetInventory (8 standard + 69 vendor MIBs) | ✅ core; MIBs grow |
> | 4 | IEC 61850 (scan + SNMP merge; libiec61850 FFI behind a feature) | ✅ complete |
> | 5 | CLI + daemon + HTTP control server + plugins | ✅ complete |
> | 6 | Local inventory | 🟡 Linux complete; OS/CPU/memory/hardware/storage/software/network on Windows + macOS; peripherals pending |
> | 7 | Remote inventory (SSH modes 1–3, WinRM) | 🟡 substantial |
> | 8 | vSphere / ESX (`glpi-agent esx`, dump/dumpfile) | ✅ complete |
> | 9 | Collect, Deploy, WakeOnLan (`glpi-agent wakeup`) | ✅ complete |
> | 10 | Stabilization + packaging | 🟡 integration / parity tests done; release installers for all 3 platforms (see below) |
>
> The `glpi-agent` CLI exposes `inventory`, `netdiscovery`, `netinventory`, `remoteinventory`, `esx`,
> `wakeup`, `inject` and `daemon`. Every fallible network/OS boundary sits behind a seam with an
> in-memory mock, so the whole suite runs offline; a **golden-file parity harness**
> (`glpi-core/tests/golden.rs`, `crates/glpi-inventory-local/tests/glpi_schema.rs`, and the
> cross-crate `glpi-agent-tests` crate) locks the GLPI wire format. The test-suite parity audit map
> lives in [tests/PARITY.md](tests/PARITY.md).
>
> Installers are built by [.github/workflows/release.yml](.github/workflows/release.yml) on a `v*`
> tag — Linux `.deb` / `.rpm` / `.tar.gz` / `.AppImage` plus Snap and Flatpak, Windows `.msi` (WiX)
> and macOS `.pkg`, for x86_64 and aarch64.
>
> Deferred to platform-specific / later phases: Windows/macOS inventory categories and certificate
> stores, the libiec61850 link (off by default, behind the `libiec61850` feature), and a static
> libiec61850 bundled per platform.
>
> See [glpi-agent-crates-summary.md](glpi-agent-crates-summary.md) for the crate map and
> [glpi-agent-rust-migration-plan.md](glpi-agent-rust-migration-plan.md) for the phased plan.

## Building

The toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml) (Rust **1.96.0**). From the repo
root:

```bash
cargo build      # build all crates
cargo test       # unit + integration + doctests
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run a single crate or a single test by name:

```bash
cargo test -p glpi-core
cargo test -p glpi-transport basic_auth_header_is_sent
```

Lints are enforced workspace-wide (`clippy::all = warn`, `clippy::suspicious = deny`,
`missing_docs = warn`), and the same checks run in CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)).

## Development environment

A devcontainer / Docker Compose setup in [`.devcontainer/`](.devcontainer/) runs a full GLPI stack as
a reference environment. Open the repo in VS Code and reopen in the container, or drive it manually:

```bash
cd .devcontainer

# Bring up GLPI + MySQL (the upstream Perl agent is NOT in runServices)
docker compose up -d

# GLPI UI
open http://localhost:8080
```

Before agents can report, enable inventory in the GLPI UI under **Setup / Settings → Inventory**
(see [.devcontainer/README.md](.devcontainer/README.md)). From inside the devcontainer network the
server is reachable at `http://glpi/front/inventory.php`, which can be used for live transport smoke
tests (the test suite itself stays offline via mocks).

### Reference agent commands

```bash
docker compose up -d agent                       # start the upstream Perl agent
docker compose logs -f agent                     # tail its inventory runs
docker compose exec agent glpi-inventory --json  # inventory as JSON, not sent to the server
docker compose down                              # tear down (-v also drops data volumes)
```

## Architecture

Four services share the `glpi_network` bridge (defined in
[.devcontainer/docker-compose.yml](.devcontainer/docker-compose.yml)):

| Service        | Role                                                                  |
| -------------- | -------------------------------------------------------------------- |
| `glpi`         | GLPI server (`glpi/glpi:latest`), host `:8080` → container `:80`     |
| `mysql`        | MySQL database backing GLPI                                          |
| `agent`        | Upstream Perl glpi-agent (the reference implementation being ported) |
| `devcontainer` | Rust dev environment VS Code attaches to                            |

The Rust agent is a Cargo workspace under [`crates/`](crates/): `glpi-core` and `glpi-transport` at
the base, then the task crates (inventory, discovery, remote inventory, vSphere, collect, deploy,
wake-on-LAN), the daemon/server crates (scheduler, HTTP, plugins), and the `glpi-cli` binary
(published as `glpi-agent`). Full details, dependencies, and risk areas are in
[glpi-agent-crates-summary.md](glpi-agent-crates-summary.md).

## AI Agent Support

This project is optimized for **any AI coding agent** (Claude Code, GitHub Copilot, Cursor, Devstral, etc.):

- **Universal guidelines**: [AGENTS.md](AGENTS.md)
- **Human developer guidelines**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **Agent-specific configurations**: [.agents/](.agents/)

Each agent has its own configuration file in the `.agents/` directory with tool-specific instructions.

## License

Licensed under the **GNU General Public License v2.0** — see [LICENSE](LICENSE), matching the upstream
GLPI agent this project ports. Every source file carries an SPDX header.
