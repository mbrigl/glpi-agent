# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Language Policy

**Everything in this repository must be in English — without exception.**

This applies to:
- All source code comments (`//`, `///`, `//!`)
- All log messages (`tracing::info!`, `tracing::warn!`, `tracing::debug!`, `tracing::error!`)
- All error messages and user-facing strings
- All documentation (README, plan files, inline docs)
- All commit messages
- All new files created in this repository

Do not write in any other language in any code or documentation. If you encounter existing text in a different language, translate it to English.


## Current state

This repo (`glpi-agent`) is a **Rust rewrite of the Perl glpi-agent**, in early **Phase 1** (Foundation). A Cargo **workspace** under `crates/` already holds all 14 member crates; the two base crates are being filled in, the rest are still placeholder skeletons (a `crate_name()` smoke-test symbol + one test).

Implemented so far:

- **`glpi-core`** — the foundation crate:
  - `error` — `AgentError` (thiserror) + `Result` alias,
  - `types` — protocol-agnostic value types (`Device`/`AssetType`, `MacAddress`/`NetworkInterface`, `SnmpCredentials` & co., `InventoryCategory`/`InventoryResult`),
  - `config` — layered `Options` / `PartialOptions` with precedence merge (`Options::resolve`); source parsers in `config::sources` (agent.cfg `key = value` format, `conf.d/*.cfg`, `GLPI_AGENT_*` env vars) and a `Loader` that assembles them in order. The Windows registry source is still TODO.
  - `protocol::glpi` — GLPI native JSON `contact`/`inventory` envelope; `protocol::fusion` — FusionInventory XML (`<REQUEST>`/`<QUERY>`/`<CONTENT>`, via quick-xml); `protocol::partial` — `no-category`/`required-category` selection,
  - `logging` — a `Logger` facade with stderr / file / callback backends.
- **`glpi-transport`** — `GlpiClient` + `GlpiClientBuilder`: reqwest (rustls) HTTP client for the `contact` handshake and inventory submission, with Basic auth, TLS options (`ca-cert-file`, client certificate for mutual TLS, `no-ssl-check`, request timeout) and status→error mapping; covered by `wiremock` integration tests.

Still TODO in Phase 1: OAuth2 and SSL fingerprint pinning, the Windows registry config source, the `glpi-injector`, logging `syslog` backend + `tracing` bridge, and the golden-file parity harness against the Perl agent. The remaining task/daemon crates come in later phases.

The authoritative design is in [glpi-agent-crates-summary.md](../glpi-agent-crates-summary.md) (crate map) and the phased plan in [glpi-agent-rust-migration-plan.md](../glpi-agent-rust-migration-plan.md) — read them before adding code (see "Planned Rust architecture" below). The devcontainer is configured for **Rust** (Rust base image, `rust-analyzer` / LLDB extensions, `formatOnSave`).

## Environment architecture

The runtime environment is defined entirely in [.devcontainer/docker-compose.yml](../.devcontainer/docker-compose.yml). Four services share the `glpi_network` bridge:

- **glpi** — the GLPI server (`glpi/glpi:latest`), host `:8080` → container `:80`. Inventory must be enabled in the GLPI UI under *Setup / Settings → Inventory* before agents can report (see [.devcontainer/README.md](../.devcontainer/README.md)).
- **mysql** — MySQL backing GLPI, host `:3306`. Credentials come from [.devcontainer/.env](../.devcontainer/.env) (`GLPI_DB_*`). Note: `.env` sets `GLPI_DB_HOST=db` but the compose service is named `mysql` — reconcile these if GLPI can't reach the DB.
- **agent** — built from [.devcontainer/Dockerfile](../.devcontainer/Dockerfile) (Debian 12 + the upstream Perl glpi-agent **1.17** installer). Loops `glpi-agent --no-fork --debug` then `sleep 3600`, reporting to `GLPI_SERVER` (`http://glpi/front/inventory.php`, resolved over the shared network). This is the reference implementation being ported to Rust.
- **devcontainer** — the Rust dev environment VS Code attaches to (`mcr.microsoft.com/devcontainers/rust:latest`); mounts the repo at `/workspace`.

Important wiring details:
- [devcontainer.json](../.devcontainer/devcontainer.json) `runServices` only auto-starts **glpi** and **mysql** — the **agent** service is **not** started with the devcontainer. Bring it up explicitly with `docker compose up -d agent`.
- The host `~/.claude` directory is bind-mounted into the devcontainer at `/home/vscode/.claude`.
- Docker is available inside the devcontainer via `docker-outside-of-docker` (talks to the host daemon), so `docker compose` commands run against the host.
- The `glpi-agent` / `glpi-inventory` Perl binaries exist **only inside the agent container**, not in the devcontainer. Run them with `docker compose exec agent …` (or `docker compose run`).

## Common commands

Run from `.devcontainer/` (where the compose file and `.env` live):

```bash
# Bring up the full stack (GLPI + DB + agent — agent is NOT in runServices)
docker compose up -d

# Rebuild the agent image after changing the Dockerfile
docker compose build agent

# Tail agent logs (inventory runs print here in debug mode)
docker compose logs -f agent

# Run an inventory as JSON without sending it to the server (runs inside agent container)
docker compose exec agent glpi-inventory --json

# Tear everything down (add -v to also drop the glpi_data / glpi_mysql volumes)
docker compose down
```

GLPI UI: http://localhost:8080.

## Building the Rust workspace

The toolchain is pinned in [rust-toolchain.toml](../rust-toolchain.toml) (currently **1.96.0**; the plan's 1.75 is too old — the dependency tree needs edition 2024). Run from the repo root:

```bash
cargo build                 # build all crates
cargo test                  # run all tests (unit + integration + doctests)
cargo fmt --check           # formatting (CI gate)
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run a single crate's tests, or one test by name:

```bash
cargo test -p glpi-core                       # one crate
cargo test -p glpi-core config::               # one module
cargo test -p glpi-transport basic_auth_header_is_sent   # one test
```

Lints are enforced workspace-wide via `[workspace.lints]` in the root `Cargo.toml`: `clippy::all = "warn"`, `clippy::suspicious = "deny"`, plus `unsafe_code`/`missing_docs = "warn"` — so every public item needs a doc comment. CI ([.github/workflows/ci.yml](../.github/workflows/ci.yml)) runs fmt + clippy (`-D warnings`) + build + test. Test stack: `rstest`, `insta` (snapshots), `wiremock` (HTTP mocks); `proptest`/`assert_cmd` to come.

The reachable GLPI server (`http://glpi/front/inventory.php` from inside the devcontainer network) can be used for live transport smoke tests; mock-based tests are the default so the suite stays offline-capable.

## Planned Rust architecture

[glpi-agent-crates-summary.md](../glpi-agent-crates-summary.md) is the authoritative design doc. Highlights to know before writing code:

- A Cargo **workspace** under `crates/` with ~14 member crates. Layering: `glpi-core` (types, protocol, config, auth, logging) and `glpi-transport` (reqwest HTTP) at the base; task crates (`glpi-inventory-local`, `glpi-discovery`, `glpi-inventory-remote`, `glpi-vsphere`, `glpi-collect`, `glpi-deploy`, `glpi-wakeonlan`), daemon/server crates (`glpi-scheduler`, `glpi-http`, `glpi-plugins`), and the `glpi-cli` binary (published as the `glpi-agent` binary) on top.
- Async on **tokio**; `reqwest` (rustls) for HTTP, `axum` for the embedded ToolBox server, `clap` for the CLI, `serde`/`quick-xml` for the JSON/XML protocol, `tracing` for logging, `thiserror`/`anyhow` for errors.
- Feature-gated optional deps: `russh` (SSH remote inventory), `wmi` + `windows`/`winreg` (Windows, must run on a dedicated COM worker thread — the `wmi` crate is `!Send`), `libiec61850-sys` (IEC 61850, FFI via `bindgen`).
- Known high-risk areas: **SNMPv3 USM** auth/priv must be hand-implemented over `hmac`/`sha2`/`aes`/`des`/`cfb-mode` (the `rasn-snmp` crate only covers wire types); config layering (Registry/conf.d) and the ping-with-TCP-fallback are also custom.
- Versioning: the Rust agent starts at **v2.0.0** to separate it from the Perl 1.x line.

The workspace is now in place; see "Building the Rust workspace" above for the `cargo build` / `test` / `fmt` / `clippy` workflow and the single-test invocation.

## Notes

- The project is licensed **GPL-2.0** ([LICENSE](../LICENSE)), matching the upstream GPL agent this ports. New Rust source files should carry an SPDX header (`// SPDX-License-Identifier: GPL-2.0-only`).
- The agent's GLPI server URL and tag are set via the `GLPI_SERVER` / `GLPI_TAG` env vars (Dockerfile defaults + compose overrides); change them there rather than hardcoding.
- The upstream agent version is pinned to 1.17 in the Dockerfile download URL — bump it there to track a new release.
