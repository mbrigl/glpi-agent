# ADR-004: Configuration Layering with Custom Sources

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent must support **layered configuration** with the following precedence (highest to lowest):

1. CLI arguments (`--server`, `--debug`, etc.)
2. Environment variables (`GLPI_AGENT_*`)
3. Configuration directory files (`conf.d/*.cfg`)
4. Main configuration file (`agent.cfg`)
5. Built-in defaults

The Rust `config` crate (v0.14) provides TOML and environment variable support but **does not natively support**:
- Custom file formats (Perl agent's `key = value` format)
- Configuration directory merging (`conf.d/*.cfg`)
- Windows Registry as a configuration source

## Decision Options

1. **Use `config` Crate Only**
   - Rely on TOML format for all configuration
   - Requires users to migrate from Perl's format
   - Lacks directory merging support

2. **Fork `config` Crate**
   - Add needed features upstream
   - High maintenance burden

3. **Custom Configuration System**
   - Implement from scratch
   - Full control over features

4. **Hybrid Approach: `config` Crate + Custom Sources**
   - Use `config` for core functionality
   - Implement custom sources for Perl-compatible formats

## Decision

We chose **Hybrid Approach: `config` Crate + Custom Sources**, because:

- **Compatibility**: Maintains full compatibility with Perl agent's configuration format
- **Leverage existing work**: Uses `config` crate's robust TOML and environment variable parsing
- **Extensibility**: Custom sources can be added for Windows Registry and other platforms
- **Precedence control**: Custom `Loader` implements the exact precedence rules from Perl agent

## Implementation

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Options (Final)                        │
└─────────────────────────────────────────────────────────┘
                              ▲
                              │
┌─────────────────────────────────────────────────────────┐
│                   Loader::load()                          │
│  - Merges sources in precedence order                      │
│  - Handles conflicts (higher precedence wins)            │
└─────────────────────────────────────────────────────────┘
                              ▲
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ CLI Args     │ │ Env Vars     │ │ Config Files │
│ (clap)       │ │ (config)     │ │ (custom)     │
└─────────────┘ └─────────────┘ └─────────────┘
```

### Key Components

1. **`PartialOptions` and `Options`**: Structs representing configuration state
2. **`Source` trait**: Unified interface for all configuration sources
3. **Custom Sources**:
   - `AgentCfgSource`: Parses Perl-style `key = value` files
   - `ConfDirSource`: Loads and merges all `*.cfg` files from a directory
   - `EnvSource`: Wraps `config` crate's env var support
   - `RegistrySource` (future): Windows Registry support
4. **`Loader`**: Orchestrates source loading and merging

### File Format Support

```
# agent.cfg (Perl-compatible format)
tag = my-tag
server = https://glpi.example.com/front/inventory.php

# conf.d/network.cfg
discovery-ranges = 192.168.1.0/24
snmp-community = public
```

## Consequences

### Positive

- **100% compatibility** with Perl agent configuration files
- **No migration required** for existing users
- **Flexible**: Can add new sources without breaking changes
- **Testable**: Each source can be unit-tested independently

### Negative

- **Additional code**: Custom sources add ~500 lines of configuration code
- **Maintenance**: Custom parsing logic for Perl-style format
- **Complexity**: Understanding the precedence rules requires documentation

## Alternatives Considered

- **`config` Crate Only**: Would require users to rewrite all configuration files in TOML format.
- **Fork `config` Crate**: The features we need are too specific to justify upstreaming.
- **Custom Configuration System**: Would duplicate much of `config` crate's functionality.
