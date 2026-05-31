# glpi-agent

A **Rust rewrite** of the [GLPI inventory agent](https://github.com/glpi-project/glpi-agent). The
upstream agent is written in Perl; this project re-implements it as a Cargo workspace of focused
crates while staying compatible with the GLPI inventory protocol.

> **Status: early scaffold.** There is no Rust source code yet. The repository currently ships a
> devcontainer that runs a full GLPI stack (server + database + the upstream Perl agent) as a
> reference environment, plus the design blueprint for the port. See
> [glpi-agent-crates-summary.md](glpi-agent-crates-summary.md) for the planned crate layout.

## Getting started

Everything runs through the devcontainer / Docker Compose setup in [`.devcontainer/`](.devcontainer/).
Open the repo in VS Code and reopen in the container, or drive the stack manually:

```bash
cd .devcontainer

# Bring up GLPI, MySQL, and the reference agent
docker compose up -d

# GLPI UI
open http://localhost:8080
```

Before agents can report, enable inventory in the GLPI UI under **Setup / Settings → Inventory**
(see [.devcontainer/README.md](.devcontainer/README.md)).

### Useful commands

```bash
# Tail the reference agent's inventory runs (debug mode)
docker compose logs -f agent

# Run an inventory as JSON without sending it to the server
docker compose exec agent glpi-inventory --json

# Rebuild the agent image after editing the Dockerfile
docker compose build agent

# Tear everything down (-v also drops the data volumes)
docker compose down
```

## Architecture

Four services share the `glpi_network` bridge (defined in
[.devcontainer/docker-compose.yml](.devcontainer/docker-compose.yml)):

| Service        | Role                                                                       |
| -------------- | -------------------------------------------------------------------------- |
| `glpi`         | GLPI server (`glpi/glpi:latest`), host `:8080` → container `:80`           |
| `mysql`        | MySQL database backing GLPI                                                 |
| `agent`        | Upstream Perl glpi-agent (the reference implementation being ported)       |
| `devcontainer` | Rust dev environment VS Code attaches to                                    |

The planned Rust agent is a Cargo workspace under `crates/` (core, transport, inventory, network
discovery, remote inventory, agent tasks, daemon/HTTP server, and a `glpi-cli` binary). Full details,
dependencies, and risk areas are in [glpi-agent-crates-summary.md](glpi-agent-crates-summary.md).

## License

Licensed under the **GNU General Public License v2.0** — see [LICENSE](LICENSE). This matches the
license of the upstream GLPI agent that this project ports.
