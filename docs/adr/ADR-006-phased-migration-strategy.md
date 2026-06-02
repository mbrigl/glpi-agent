# ADR-006: Phased Migration Strategy

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent is a **large, complex system** with ~200 test files and ~4,300 sub-tests in the Perl implementation. A "big bang" rewrite would:
- Take **years** to complete
- Introduce **high risk** of regressions
- Delay **delivering value** to users
- Make **testing difficult**

## Decision Options

1. **Big Bang Rewrite** - Rewrite everything before releasing anything
2. **Feature-by-Feature Replacement** - Replace Perl modules in existing codebase
3. **Phased Approach** - Divide work into logical phases
4. **Minimal Viable Product (MVP) First** - Build minimal agent first

## Decision

We chose **Phased Approach**, because:
- **Risk mitigation**: Each phase is self-contained and testable
- **Incremental delivery**: Users can benefit from completed phases early
- **Clear prioritization**: Dependencies between phases are explicit
- **Test parity**: Can validate each phase against Perl agent behavior
- **Parallel development**: Teams can work on different phases simultaneously

## Phase Definition

### Phase 1: Foundation (✅ Complete)
- **`glpi-core`**: Types, protocol, configuration, auth, logging
- **`glpi-transport`**: HTTP client, glpi-injector
- **Golden-file harness**: Test infrastructure

### Phase 2: NetDiscovery Core (✅ Complete)
- IP range expansion
- Parallel scanner
- Discovery methods: Ping, ARP, NetBIOS, SNMP
- SNMP client with async support

### Phase 3: NetInventory + MIBs (🟡 In Progress)
- MIB framework
- Standard MIBs (8): system, if, entity, printer, bridge, lldp, cdp, ip
- Vendor MIBs (69+): networking, printers, storage, PDUs/UPS, telephony, KVM, sensors and servers. Networking incl. Cisco (+Meraki/+UCS board), Juniper, Fortinet, Mikrotik, Nokia/Alcatel, D-Link (+DGS1210), Brocade, Netgear, Aruba, Aerohive, WatchGuard, Telco, FoxGate, Tiesse, Intelbras, Voltaire, TP-Link, Ubiquiti, DefencePro, Radware; printers HP, Brother, Ricoh, Konica/Sindoh, Lexmark, Xerox, Canon, Epson, OKI, Pantum, Kyocera, Toshiba, Zebra; storage EMC, Hitachi Vantara, HP Citizen; PDUs/UPS Eaton, Raritan, Bachmann, RNX, DigiPower, APC/UPS-MIB/Riello, Voltronic; telephony Avaya, Htek, Snom, Multitech; sensors/servers Akcp, Hwg, Meinberg, Siemens, SiemensSicam, CheckPoint, Wyse ThinOS, iDRAC, iLO, Avocent. A few upstream modules that mutate ports/components/SIM/process state remain deferred (CiscoPortSecurity, Force10S, Netgear, Digi, Panasas, LinuxAppliance).
- NetInventory task
- PDU support (GLPI 12+): the `PDU` device type and `PDU.PLUGS` outlet
  inventory (number / name / connector type) on a glpi-version hint threaded
  through `MibRegistry`/`NetInventoryTask` (`netinventory --glpi-version`);
  RNX and Raritan read their outlet tables into plugs, Bachmann / DigiPower
  switch type by version. (Upstream Eaton has no outlet/PDU-type logic — it is
  classified via `sysobject.ids` — so it is intentionally unchanged.)

### Phase 4: Platform-Specific Features (🟡 In Progress)
- Windows: COM/WMI worker, Registry config, Certificate store (⏳ pending)
- macOS: Keychain, System Profiler (⏳ pending)
- IEC 61850 (✅): `glpi-iec61850` ports the upstream
  `IEC61850::{Protocol,Device}` scan/inventory logic over an `IedProtocol`
  seam — first logical device → `LPHD<n>` → `PhyNam` attributes → GLPI
  inventory (INFO/ITEMTYPE/FIRMWARES, GLPI 11+ `IedAsset` itemtype, Siemens
  `A_Allg` name cleanup), fully unit-tested against an in-memory mock IED. The
  on-wire MMS transport (libiec61850 FFI, or a pure-Rust MMS client) plugs into
  `IedProtocol` behind the off-by-default `libiec61850` feature; it is not built
  by default since libiec61850 + a C toolchain are not assumed present.

### Phase 5: CLI + Daemon (🟡 Partially Complete)
- `glpi-agent` binary with subcommands
- Configuration loading
- Category filtering
- `glpi-scheduler` and `glpi-http`
- Event system (`glpi-scheduler::event`): the typed `Event` (init / runnow /
  taskrun / partial / maintenance / job) with `from_params` and serde
- Task-fork IPC (`glpi-scheduler::ipc`): length-prefixed `IpcMessage` frames
  (Event / Log / Progress / Result / Done) over any async stream, handling
  arbitrarily long messages
- Task-fork process spawn (`glpi-scheduler::fork`): the parent (`TaskWorker`)
  spawns a worker command with piped stdio, sends it the `Event` and streams
  back its messages to completion; the child halves (`read_initial_event` +
  `WorkerReporter`) read the event off stdin and report progress/results on
  stdout. Wired in the CLI: a hidden `__task-worker` subcommand runs a task
  (`selftest`, `netdiscovery`) over IPC, and `daemon --fork-tasks` runs each
  scan in such a child instead of inline — verified by an end-to-end test that
  spawns the real `glpi-agent` binary and round-trips the protocol over OS pipes
- HTTP-server plugins (`glpi-plugins`): the Proxy plugin (config + store/forward
  + pass-through loop guard) and the SSL plugin (HTTPS-listener config +
  validation)
- Plugins mounted in `glpi-http`: the Proxy route (`HttpServer::with_proxy`)
  receives a POSTed inventory on the plugin's `url_path`, applies the plugin's
  `plan` (loop-guard / trust refusal, local store keyed by device id, forward to
  the configured GLPI servers through an `InventoryForwarder` →
  `TransportForwarder`); the SSL plugin's HTTPS listener (`tls::server_config`
  builds a rustls config from the PEM cert/key, `tls::serve_tls` TLS-terminates
  and serves the control router with the peer address injected so the trust
  middleware still applies) — done, including an end-to-end HTTPS handshake test
- Event wiring: the embedded server maps each `/now` request (`partial`, `full`,
  `task`, `category`, `delay`) to a typed `Event` and delivers it to the daemon
  over the trigger channel; the daemon logs the event kind/task and honours its
  `delay` and task target — done
- Still pending: ToolBox UI pages, and the rest of the daemon lifecycle
  (background fork/detach + PID file, `conf-reload-interval`), plus wiring the
  remaining real tasks (inventory, …) into the `__task-worker` entry point

### Phase 6: Local Inventory (🟡 Linux Complete)
- Linux: All 20+ categories
- Windows: All categories (WMI-based)
- macOS: All categories
- Other platforms: Solaris, HP-UX, AIX, FreeBSD

### Phase 7: Remote Inventory (✅ Complete)
- `glpi-inventory-remote`: target model (`ssh://`/`winrm://`), `RemoteSession`
  seam (reuses the local parsers verbatim), **SSH mode 1** (command-line `ssh`),
  **SSH mode 2** (pure-Rust `russh` transport, `ring` backend, behind the default
  `russh` feature: password + key auth, TOFU host keys, channel exec),
  `assetname-support`, **mode 3 (`perl`)** — remote `perl` one-liners gated on a
  capability probe (richer `Net::CUPS` printers, `Net::Domain` FQDN fallback,
  `RemoteModes` from `?mode=`), the Linux command orchestration, and the
  `remoteinventory` CLI subcommand with `--remote-workers` parallelism
  (Semaphore-bounded, optional per-host server submission) — done
- Delta / partial inventory: `glpi-core::protocol::delta` keeps per-device
  section checksums in a state file and plans a full-vs-partial submission
  (`full-inventory-postpone`); a shared `submit_inventory` helper wires it into
  both the local `inventory` and the `remoteinventory` CLI commands (per-host,
  keyed by device id) via `--statedir` / `--full-inventory-postpone` — done
- Host-key verification: russh checks the presented key against a `known_hosts`
  file, pinning new hosts Trust-On-First-Use or rejecting them under a strict
  policy; exposed on `remoteinventory` via `--known-hosts` / `--strict-host-keys`
  (also wired to the CLI transport's `UserKnownHostsFile`) — done
- WinRM (Windows transport): `WinRmSession` speaks WS-Management + the WinRS
  shell (Create → Command → Receive loop → Signal → Delete) over HTTP(S) with
  HTTP Basic auth, behind the default `winrm` feature; the SOAP envelope
  builders and response parsers are pure/unit-tested. Selected by
  `remoteinventory --transport winrm` — done
- Follow-ups (outside Phase 7): WinRM NTLM/Negotiate/Kerberos auth, and the
  Windows-specific collection command set (Phase 6b) that lets a WinRM host
  produce a full inventory rather than just transport-level command execution
- `glpi-vsphere`: VMware ESX/vCenter
- State files and delta diff

## Test Migration

**Critical Rule**: Every test must be migrated alongside the module it covers.
- Reuse Perl agent's fixtures
- Golden-file comparison against Perl output
- Phase is "done" only when tests pass

## Consequences

### Positive
- Early feedback from each phase
- Risk reduction through isolation
- Parallel development possible
- Clear progress milestones

### Negative
- Delayed full feature set
- Integration challenges between phases
- Test maintenance overhead

## Alternatives Considered

- **Big Bang Rewrite**: Too risky with no intermediate deliverables.
- **Feature-by-Feature**: Complex Perl/Rust integration overhead.
- **MVP First**: Early architectural decisions might need revision.
