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
