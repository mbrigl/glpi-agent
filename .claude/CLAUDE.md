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

This repo (`glpi-agent`) is an early-stage scaffold. There is **no application source code yet** — only a devcontainer + Docker Compose environment that runs GLPI and the upstream [glpi-agent](https://github.com/glpi-project/glpi-agent) inventory agent. The devcontainer is configured for **Rust** development (Rust base image, `rust-analyzer` / LLDB recommended extensions, `formatOnSave`), so new code is expected to be Rust.

The intended work is a **Rust rewrite of the Perl glpi-agent**. The planned design is captured in [glpi-agent-crates-summary.md](../glpi-agent-crates-summary.md) — read it before adding code (see "Planned Rust architecture" below).

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

## Planned Rust architecture

[glpi-agent-crates-summary.md](../glpi-agent-crates-summary.md) is the authoritative design doc. Highlights to know before writing code:

- A Cargo **workspace** under `crates/` with ~14 member crates. Layering: `glpi-core` (types, protocol, config, auth, logging) and `glpi-transport` (reqwest HTTP) at the base; task crates (`glpi-inventory-local`, `glpi-discovery`, `glpi-inventory-remote`, `glpi-vsphere`, `glpi-collect`, `glpi-deploy`, `glpi-wakeonlan`), daemon/server crates (`glpi-scheduler`, `glpi-http`, `glpi-plugins`), and the `glpi-cli` binary (published as the `glpi-agent` binary) on top.
- Async on **tokio**; `reqwest` (rustls) for HTTP, `axum` for the embedded ToolBox server, `clap` for the CLI, `serde`/`quick-xml` for the JSON/XML protocol, `tracing` for logging, `thiserror`/`anyhow` for errors.
- Feature-gated optional deps: `russh` (SSH remote inventory), `wmi` + `windows`/`winreg` (Windows, must run on a dedicated COM worker thread — the `wmi` crate is `!Send`), `libiec61850-sys` (IEC 61850, FFI via `bindgen`).
- Known high-risk areas: **SNMPv3 USM** auth/priv must be hand-implemented over `hmac`/`sha2`/`aes`/`des`/`cfb-mode` (the `rasn-snmp` crate only covers wire types); config layering (Registry/conf.d) and the ping-with-TCP-fallback are also custom.
- Versioning: the Rust agent starts at **v2.0.0** to separate it from the Perl 1.x line.

When the workspace is created, wire up the standard `cargo build` / `cargo test` / `cargo fmt` / `cargo clippy` workflow and document the single-test invocation (e.g. `cargo test -p <crate> <test_name>`) here. The design doc specifies `clippy` lints (`all = "warn"`, `suspicious = "deny"`) and a test stack of `rstest`, `insta` (snapshots), `wiremock`, `proptest`, `assert_cmd`.

## Notes

- The project is licensed **GPL-2.0** ([LICENSE](../LICENSE)), matching the upstream GPL agent this ports. New Rust source files should carry an SPDX header (`// SPDX-License-Identifier: GPL-2.0-only`).
- The agent's GLPI server URL and tag are set via the `GLPI_SERVER` / `GLPI_TAG` env vars (Dockerfile defaults + compose overrides); change them there rather than hardcoding.
- The upstream agent version is pinned to 1.17 in the Dockerfile download URL — bump it there to track a new release.
