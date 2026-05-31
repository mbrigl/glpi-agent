# GLPI Devcontainer Environment

This directory contains a complete devcontainer setup for developing and
testing GLPI with an inventory agent.

## Overview

This environment provides a complete GLPI instance with a MySQL database
and makes it possible to run a GLPI agent for inventory collection. The
configuration is handled via Docker Compose and is optimized for use in a
VS Code devcontainer.

## Services

The environment consists of the following services:

- **glpi**: The GLPI server, reachable on port 8080
- **mysql**: The MySQL database for GLPI
- **agent**: The GLPI inventory agent (Perl-based)
- **devcontainer**: The Rust development environment

## Setup

### 1. Prerequisites

Make sure that Docker and Docker Compose are installed.

### 2. Start the environment

The environment is started automatically when the devcontainer is opened.
Alternatively, the environment can be started manually:

```bash
docker compose up -d
```

### 3. Configure GLPI

Open GLPI in the browser at http://localhost:8080 and log in with the
default credentials:

- Username: `glpi`
- Password: `glpi`

**Important**: Enable inventory in GLPI under
*Setup / Settings → Inventory* before agents can report data.

### 4. Start the agent

The agent is not part of `runServices` and must be started manually:

```bash
docker compose exec agent glpi-agent --server http://glpi/front/inventory.php
```

## Useful commands

```bash
# Show the agent's logs
docker compose logs -f agent

# Output the inventory as JSON (without sending it to the server)
docker compose exec agent glpi-inventory --json

# Shut down the environment
docker compose down
```

## Troubleshooting

### GLPI cannot reach the database

The `.env` file sets `GLPI_DB_HOST=db`, but the Compose service is named
`mysql`. Correct this if GLPI cannot reach the database.

### Agent does not report any data

Make sure that inventory is enabled in GLPI (see setup step 3) and that
the agent is configured correctly.
