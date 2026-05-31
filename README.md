# glpi-agent

A **Rust rewrite** of the [GLPI inventory agent](https://github.com/glpi-project/glpi-agent). The
upstream agent is written in Perl; this project re-implements it as a Cargo workspace of focused
crates while staying compatible with the GLPI inventory protocol. It starts at **v2.0.0** to
separate it from the Perl 1.x line.

> **Status: Phase 1 (Foundation), in progress.** The Cargo workspace and all 14 member crates exist.
> The two base crates are being filled in; the task/daemon crates are still placeholder skeletons.
>
> - **`glpi-core`** — `error`, `types` (device / network / SNMP / inventory), `config` (layered
>   options + precedence merge), `protocol::glpi` (native JSON `contact`/`inventory`) + category
>   filtering, and `logging` (stderr / file / callback backends). _Implemented & tested._
> - **`glpi-transport`** — `GlpiClient`, a reqwest (rustls) HTTP client for the `contact` handshake
>   and inventory submission, with Basic auth and error mapping. _Implemented & tested (wiremock)._
> - Everything else (`glpi-discovery`, `glpi-inventory-local`, `glpi-vsphere`, `glpi-cli`, …) is a
>   skeleton awaiting its phase.
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

## License

Licensed under the **GNU General Public License v2.0** — see [LICENSE](LICENSE), matching the upstream
GLPI agent this project ports. Every source file carries an SPDX header.
