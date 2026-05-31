# GLPI Agent Rust — Crate Overview

## 1. Internal Workspace Crates (17 total)

### A — Core (2 crates)
| Crate | Contents |
|---|---|
| `glpi-core` | Types, protocol (JSON/XML), configuration, auth (Basic/OAuth2/SSL/Keystore), logging |
| `glpi-transport` | HTTP client (reqwest), glpi-injector |

### B — Local Inventory (1 crate)
| Crate | Contents |
|---|---|
| `glpi-inventory-local` | All inventory categories for Linux, Windows, macOS, Solaris, HP-UX, AIX, FreeBSD |

### C — Network Discovery (2 crates)
| Crate | Contents |
|---|---|
| `glpi-discovery` | NetDiscovery task, NetInventory task, SNMP (v1/v2c/v3), IP scanner, 8 standard MIBs + 35 vendor MIBs, ARP, NetBIOS, Ping |
| `glpi-iec61850` | IEC 61850 / OT devices, libiec61850 FFI binding |

### D — Remote Inventory (2 crates)
| Crate | Contents |
|---|---|
| `glpi-inventory-remote` | SSH (3 modes), WinRM, Remote Inventory task, state files/delta diff |
| `glpi-vsphere` | VMware ESX/vCenter SOAP client, ESX task |

### E — Agent Tasks & Daemon (10 crates)
| Crate | Contents |
|---|---|
| `glpi-collect` | Collect task v3.0: Registry, WMI, file reading, SHA-256/512 |
| `glpi-deploy` | Deploy task v3.5: HTTP download, P2P mirror, SHA-512 verification, installers/scripts |
| `glpi-wakeonlan` | WakeOnLan task: Magic Packet UDP broadcast |
| `glpi-scheduler` | Daemon scheduling, events, task forking (Unix/Windows), IPC |
| `glpi-http` | Embedded HTTP server (port 62354), ToolBox v1.7, API (/status, /now, /) |
| `glpi-plugins` | HTTP server plugin trait, Proxy plugin v3.0, SSL plugin v2.0 |
| `glpi-cli` | CLI binary with subcommands (netdiscovery, netinventory, inventory, esx, remoteinventory, inject, wakeup, daemon) |
| `test-fixtures` (optional) | Shared test-data library (imports resources/) |

---

## 2. External Dependencies (by category)

### Async Runtime (3)
| Crate | Version | Use |
|---|---|---|
| `tokio` | 1.x | Core runtime, parallelity, I/O |
| `async-trait` | 0.1 | Async trait definitions |
| `tokio-stream` | 0.1 | Stream utilities, async iteration |

### Concurrency & sync (2)
| Crate | Version | Use |
|---|---|---|
| `futures` | 0.3 | Future combinators, channels |
| `tokio-cron-scheduler` | 0.10 | Scheduling, jitter, backoff |

### HTTP & Web (3)
| Crate | Version | Use |
|---|---|---|
| `reqwest` | 0.12 | HTTP client (TLS, compression, retry) |
| `axum` | 0.7 | HTTP server framework (ToolBox) |
| `quick-xml` | 0.36 | XML parsing/serialization (FusionInventory, vSphere SOAP) |

### SNMP Stack (6)
| Crate | Version | Use |
|---|---|---|
| `rasn` | latest | ASN.1 BER/DER codec (underlying) |
| `rasn-snmp` | latest | SNMP message types (v1/v2c/v3 encode/decode only) |
| `rasn-smi` | latest | SMI object types |
| `hmac` | latest | HMAC auth (SNMPv3 USM, custom impl.) |
| `sha1`, `sha2` | latest | SHA-1/224/256/384/512 (SNMPv3) |
| `aes`, `des`, `cfb-mode` | latest | AES/DES priv (SNMPv3 USM, custom impl.) |

### Network & Discovery (3)
| Crate | Version | Use |
|---|---|---|
| `ping-rs` | latest | ICMP ping (unprivileged DGRAM + fallback) |
| `socket2` | 0.5 | Raw/DGRAM socket control |
| `ipnetwork` | 0.20 | IP-range iteration (CIDR, range) |

### Serialization (3)
| Crate | Version | Use |
|---|---|---|
| `serde` | 1.x | Serialization framework |
| `serde_json` | 1.x | JSON |
| `serde_xml_rs` | 0.6 | FusionInventory XML (legacy) |

### Configuration (2)
| Crate | Version | Use |
|---|---|---|
| `config` | 0.14 | TOML + env base (Registry/conf.d = custom) |
| `notify` | 6.x | File-change watcher (conf-reload-interval) |

### CLI & Argument Parsing (1)
| Crate | Version | Use |
|---|---|---|
| `clap` | 4.x | CLI argument parsing, subcommands |

### Logging (3)
| Crate | Version | Use |
|---|---|---|
| `tracing` | 0.1 | Structured logging framework |
| `tracing-subscriber` | 0.3 | Logging backends (stderr, file, syslog) |
| `tracing-appender` | optional | File + rotating-file appender |

### Error Handling (2)
| Crate | Version | Use |
|---|---|---|
| `thiserror` | 1.x | Error types (`#[error]` derive) |
| `anyhow` | 1.x | Dynamic errors (binaries/main) |

### Remote Access (2)
| Crate | Version | Use |
|---|---|---|
| `russh` | 0.43 | SSH client (libssh2 replacement) |
| `wmi` | 0.13 | WMI queries (Windows only; use via COM worker) |

### Windows-specific (2)
| Crate | Version | Use |
|---|---|---|
| `windows` | 0.58 | Win32 API (CNG, KeyStore, Registry, WMI, PowerShell) |
| `winreg` | 0.52 | Registry access (wrapper) |

### Cryptography & Hashing (2)
| Crate | Version | Use |
|---|---|---|
| `sha2` | 0.10 | SHA-256/512 (Deploy/Collect checksums) |
| `base64` | latest | Base64 encoding (auth, some protocols) |

### FFI & C Integration (2)
| Crate | Version | Use |
|---|---|---|
| `libiec61850-sys` | 1.6 | IEC 61850 C library FFI (optional) |
| `bindgen` | 0.70 | C FFI code generation (build-dep) |

### Utilities (3)
| Crate | Version | Use |
|---|---|---|
| `mac_address` | 1.x | MAC address handling (WakeOnLan) |
| `uuid` | 1.x | UUID generation (inventory IDs) |
| `chrono` | 0.4 | Timestamps, timezone handling |

### Testing (6) — see §13
| Crate | Version | Use |
|---|---|---|
| `rstest` | 0.18 | Parameterized tests (fixture matrix) |
| `insta` | latest | Snapshot/golden-file assertions |
| `wiremock` | latest | Mock HTTP server (GLPI, SOAP) |
| `proptest` | latest | Property-based testing |
| `cargo-llvm-cov` | latest | Coverage measurement |
| `assert_cmd` + `predicates` | latest | CLI black-box tests |

---

## 3. Summary

| Category | Count | Details |
|---|---|---|
| **Internal crates** | 17 | 2 Core, 1 Local, 2 Discovery, 2 Remote, 10 Tasks/Daemon |
| **External runtime** | 5 | tokio, async-trait, tokio-stream, futures, tokio-cron-scheduler |
| **HTTP/Web** | 3 | reqwest, axum, quick-xml |
| **SNMP Stack** | 6 | rasn, rasn-snmp, rasn-smi, hmac, sha{1,2}, aes/des (custom USM) |
| **Network** | 3 | ping-rs, socket2, ipnetwork |
| **Serialization** | 3 | serde, serde_json, serde_xml_rs |
| **Config/Watch** | 2 | config, notify |
| **Logging** | 3 | tracing, tracing-subscriber, tracing-appender |
| **Error** | 2 | thiserror, anyhow |
| **Remote** | 2 | russh, wmi |
| **Windows** | 2 | windows, winreg |
| **Crypto/Hash** | 2 | sha2, base64 |
| **FFI** | 2 | libiec61850-sys, bindgen |
| **Utilities** | 3 | mac_address, uuid, chrono |
| **Testing** | 6 | rstest, insta, wiremock, proptest, cargo-llvm-cov, assert_cmd |
| **CLI** | 1 | clap |
| **TOTAL** | ~49 | External dependencies (exact count varies by feature flag) |

---

## 4. Feature-Flag-Dependent Dependencies

```toml
[dependencies]
# Always present
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
axum = "0.7"
clap = { version = "4", features = ["derive"] }
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

# Conditional
[dependencies.russh]
version = "0.43"
optional = true

[dependencies.libiec61850-sys]
version = "1.6"
optional = true

[dependencies.wmi]
version = "0.13"
optional = true

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Win32_System_Com", "Win32_System_Ole", "Win32_Security_Cryptography"] }
winreg = "0.52"

[dev-dependencies]
rstest = "0.18"
insta = { version = "1", features = ["json"] }
wiremock = "0.5"
proptest = "1"
tokio = { version = "1", features = ["full"] }
```

---

## 5. Highlights & Critical Crates

### High-Risk
- **SNMP Stack:** `rasn-snmp` only covers data types. SNMPv3 USM auth/priv (SHA-512, AES256C) **must be implemented in-crate** using `hmac`, `sha2`, `aes`, `des`, `cfb-mode`. Phase 2 risk #1.
- **WMI:** The `wmi` crate is **not Send** across Tokio tasks. It must run via a dedicated COM worker thread with an mpsc channel (Phase 6, Windows).
- **libiec61850:** FFI binding via `bindgen`; only tested on x86_64. Optional feature.

### Well-established
- `tokio` (1.x) — de-facto standard async runtime
- `reqwest` — established for HTTP
- `serde` + `serde_json` — quasi-standard
- `clap` — CLI best practice

### Custom Implementation
- **SNMPv3 USM:** The plan only allows crypto crates; the control flow and state machine must be built by hand
- **Config layering:** Registry/conf.d must be implemented explicitly via `windows`/`winreg` + custom merge logic
- **Ping fallback:** `ping-rs` + TCP fallback (no existing crate)

---

## 6. Dependency Graph (simplified)

```
glpi-cli (binary)
  ├── glpi-core (types, config, protocol, auth, logging)
  ├── glpi-transport (HTTP client)
  ├── glpi-discovery (SNMP, IP-scanner)
  │   ├── rasn / rasn-snmp / rasn-smi
  │   ├── hmac / sha{1,2} / aes / des / cfb-mode
  │   ├── ping-rs / socket2 / ipnetwork
  ├── glpi-inventory-local (hw/sw categories)
  ├── glpi-inventory-remote (SSH, WinRM)
  ├── glpi-vsphere (ESX SOAP)
  │   └── quick-xml
  ├── glpi-collect / glpi-deploy / glpi-wakeonlan
  ├── glpi-scheduler (events, task-fork)
  ├── glpi-http (HTTP server, ToolBox)
  │   └── axum
  └── glpi-iec61850 (optional)
      └── libiec61850-sys

External dependencies:
  tokio → async-trait, tokio-stream, futures, tokio-cron-scheduler
  reqwest → ... (embedded)
  clap → ... (embedded)
  windows (cfg(windows)) → Win32 APIs (CNG, WMI, Registry)
  wmi → ... (use via COM worker thread)
  russh → SSH library
```

---

## 7. Installation & Feature Matrix

```bash
# Default (Inventory + NetDiscovery + NetInventory + WakeOnLan + HTTP)
cargo build --release

# With all features
cargo build --release --all-features

# Individual features
cargo build --release --features netdiscovery,esx,remote-inventory,iec61850,collect,deploy

# Library only
cargo build -p glpi-core -p glpi-discovery --release
```

---

## 8. Cargo Workspace Structure

```
Cargo.toml (workspace root)
  [workspace]
  members = [
    "crates/glpi-core",
    "crates/glpi-transport",
    "crates/glpi-inventory-local",
    "crates/glpi-discovery",
    "crates/glpi-iec61850",
    "crates/glpi-inventory-remote",
    "crates/glpi-vsphere",
    "crates/glpi-collect",
    "crates/glpi-deploy",
    "crates/glpi-wakeonlan",
    "crates/glpi-scheduler",
    "crates/glpi-http",
    "crates/glpi-plugins",
    "crates/glpi-cli",
  ]
  resolver = "2"

  [workspace.lints.clippy]
  all = "warn"
  suspicious = "deny"

crates/glpi-core/Cargo.toml
  [package]
  name = "glpi-core"
  version = "2.0.0"  # Semantic Versioning separate from Perl 1.x

crates/glpi-cli/Cargo.toml
  [package]
  name = "glpi-agent"
  [[bin]]
  name = "glpi-agent"
  path = "src/main.rs"
```

---

## 9. Versioning Strategy

- **The Rust agent starts at v2.0.0** (not v1.17, not v1.18) → clear separation from Perl v1.x
- **Semantic Versioning:** 2.0.0 → 2.1.0 (new inventory category) → 2.1.1 (bug fix)
- **libsemver policy:** all workspace crates versioned as `workspace.version` in the root `Cargo.toml` (optional, cleaner)
