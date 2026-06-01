# GLPI Agent — Rust Migration Plan (v5, English, implementation-ready)

**Source basis:** glpi-project/glpi-agent, source code + changelog v1.7–v1.17 (May 2026)
**Scope:** Agent only, no server. Full feature parity with GLPI Agent 1.17.
**Goal:** A Rust Cargo workspace usable as a library (`lib`), a CLI, and a daemon/agent — modular and extensible.

> This document is written for implementation with Claude Code. Each crate maps to a discrete unit of work. File paths are normative: create them as written unless a technical reason requires otherwise.

---

## 0. Implementation notes (read first)

These notes capture corrected technical assumptions. Earlier drafts contained inaccuracies that are fixed here:

1. **SNMP stack: use `snmp2` (decision revised — Phase 2).** Earlier drafts assumed no crate covered the full SNMPv3 algorithm matrix, so the client was to be assembled from `rasn` + `rasn-snmp` + `rasn-smi` plus a hand-built USM (since the old `snmp_usm` covered only SHA-1 + AES-128). That assumption is **out of date**. The `snmp2` crate (v0.5, async via its `tokio` feature, pure-Rust crypto via `crypto-rust`) provides v1/v2c/v3 with:
   - the **full auth matrix**: MD5, SHA-1, SHA-224/256/384/512,
   - priv DES, AES-128, **AES-192, AES-256**,
   - both AES-192/256 key-localization methods — `KeyExtension::Blumenthal` (standard) and `KeyExtension::Reeder` (the Cisco "AES-192-C / AES-256-C" variant) — so the previously-flagged Cisco gap **is covered**.

   `snmp2` is dual-licensed `MIT OR Apache-2.0`; we **elect the MIT arm** for GPL-2.0 compatibility (Apache-2.0 is GPL-2.0-incompatible). This removes the single highest-risk item in the plan (hand-rolled USM crypto). Use `tokio::net::UdpSocket` is no longer needed directly — `AsyncSession` owns the transport. **Known `snmp2` 0.5 limitations to track:** it parses but does not let callers *set* a non-default SNMPv3 `contextName` (GLPI 1.17 feature) — revisit (upstream PR or fork) when a device requires it. We still add RFC 3414/7860 crypto-vector tests around `snmp2` as a regression guard (acceptance criterion §11).

2. **ICMP ping crate choice.** `surge-ping` requires raw sockets and therefore Administrator privileges on Windows. Prefer `ping-rs` (works without admin on both Windows and Linux) or implement a dual strategy: unprivileged DGRAM ICMP socket where supported (Linux kernel ≥ 3.0 with `net.ipv4.ping_group_range`, Windows IcmpSendEcho2 API), with a TCP-connect fallback. Document `CAP_NET_RAW` for the raw-socket path.

3. **WMI on Windows uses COM, which is apartment-threaded.** The `wmi` crate wraps `COMLibrary`/`WMIConnection`, which are **not** `Send` across arbitrary Tokio worker threads. Do not call WMI directly from `tokio::spawn` tasks. Instead, run all WMI work on a dedicated OS thread that initializes COM once (`CoInitializeEx` with the right apartment), and communicate with it over an `mpsc` channel. This mirrors the Perl agent's `_win32_ole_worker` design.

4. **`config` crate (0.14) does not read the Windows Registry or merge `conf.d/*.cfg` automatically.** Registry reading needs `windows`/`winreg`; layered `.cfg` merging with the documented precedence (defaults → agent.cfg → conf.d → environment → CLI) must be implemented explicitly.

5. **Output-format parity is the acceptance gate.** The deliverable that matters is JSON that the existing GLPI server accepts, plus FusionInventory XML. Build a golden-file test harness early: run the Perl agent and the Rust agent against identical fixtures and diff the normalized JSON.

6. **Naming:** keep the workspace name `glpi-agent`. The CLI binary is `glpi-agent` with subcommands; legacy command names (`glpi-netdiscovery`, `glpi-netinventory`, `glpi-esx`, `glpi-injector`) can be provided as thin alias binaries if needed for drop-in compatibility.

7. **Tests are migrated, not rewritten from scratch.** The Perl agent ships an extensive test suite (~200 test files, ~4300 sub-tests under `t/`, backed by real-world fixtures under `resources/`). These tests and fixtures are a primary asset: they encode hard-won knowledge of vendor quirks, malformed outputs, and edge cases gathered over a decade. **Every test must be migrated alongside the module it covers, in the same phase — not deferred to the end.** A module is "done" only when its migrated tests pass. The original fixtures (command outputs, SNMP walks, WMI dumps, EDID blobs) are reused verbatim as Rust test data. See §13 for the full test-migration strategy.

---

## 1. Module categories

All functionality is grouped into five categories:

| Cat. | Name | Description | Crates |
|---|---|---|---|
| **A** | Core | Types, protocol, config, transport, logging | `glpi-core`, `glpi-transport` |
| **B** | Local Inventory | Inventory of the local system (all platforms) | `glpi-inventory-local` |
| **C** | Network Discovery | Network scan + SNMP inventory + IEC 61850 | `glpi-discovery`, `glpi-iec61850` |
| **D** | Remote Inventory | Inventory of foreign systems (SSH / WinRM / ESX) | `glpi-inventory-remote`, `glpi-vsphere` |
| **E** | Agent Tasks & Daemon | Server-driven tasks, scheduling, HTTP, CLI | `glpi-collect`, `glpi-deploy`, `glpi-wakeonlan`, `glpi-scheduler`, `glpi-http`, `glpi-plugins`, `glpi-cli` |

---

## 2. Workspace layout

```
glpi-agent/
├── Cargo.toml                          # Workspace manifest
│
├── crates/
│   │
│   # ───────────────────────────────────────────────────
│   # A — CORE
│   # ───────────────────────────────────────────────────
│   ├── glpi-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types/
│   │       │   ├── device.rs           # Device, AssetType (GLPI 11+ genericity)
│   │       │   ├── network.rs          # NetworkInterface, IpAddress, MacAddress
│   │       │   ├── inventory.rs        # InventoryResult, InventoryCategory enum
│   │       │   └── snmp.rs             # SnmpCredentials (v1/v2c/v3 + contextname)
│   │       ├── config/
│   │       │   ├── mod.rs              # Loader with precedence: defaults <
│   │       │   │                       #   agent.cfg < conf.d/*.cfg < env < CLI
│   │       │   ├── sources.rs          # agent.cfg, conf.d/*.cfg, Windows Registry
│   │       │   ├── options.rs          # All options (see §5)
│   │       │   ├── snmp_advanced.rs    # snmp-advanced-support.cfg (edge devices)
│   │       │   └── feature_gates.rs    # glpi-version-dependent format features
│   │       ├── protocol/
│   │       │   ├── glpi.rs             # GLPI native JSON (START / DEVICE / STOP)
│   │       │   ├── fusion.rs           # FusionInventory XML (compatibility)
│   │       │   └── partial.rs          # Partial inventory / delta-diff logic
│   │       ├── auth/
│   │       │   ├── basic.rs            # HTTP Basic auth
│   │       │   ├── oauth2.rs           # OAuth2 (GLPI 11+, /front/inventory.php)
│   │       │   ├── ssl.rs              # ssl-cert-file, ssl-key-file, fingerprint,
│   │       │   │                       #   ca-cert-file, ca-cert-dir, no-ssl-check
│   │       │   ├── keystore_win.rs     # Windows certificate store / CNG (cfg(windows))
│   │       │   └── keychain_mac.rs     # macOS Keychain / system-ssl-ca (cfg(macos))
│   │       ├── logging/
│   │       │   ├── mod.rs              # Logger trait + callback API
│   │       │   ├── stderr.rs           # stderr backend
│   │       │   ├── file.rs             # file backend
│   │       │   └── syslog.rs           # syslog backend (cfg(unix))
│   │       └── error.rs                # AgentError (thiserror)
│   │
│   ├── glpi-transport/
│   │   └── src/
│   │       ├── client.rs               # reqwest HTTP client, retry, compression
│   │       ├── auth.rs                 # Basic + OAuth2 + SSL client cert
│   │       └── injector.rs             # glpi-injector logic:
│   │                                   #   --ca-cert-file, --ssl-fingerprint,
│   │                                   #   --ssl-key-file, --agentid, OAuth2
│   │
│   # ───────────────────────────────────────────────────
│   # B — LOCAL INVENTORY
│   # ───────────────────────────────────────────────────
│   ├── glpi-inventory-local/
│   │   └── src/
│   │       ├── lib.rs                  # Inventory task (impl Task trait)
│   │       ├── task.rs                 # Flow, category filtering (no-category, etc.)
│   │       │
│   │       ├── categories/             # ══ Inventory categories ══
│   │       │   │                       # Each file: is_enabled() + do_inventory()
│   │       │   │
│   │       │   # Hardware
│   │       │   ├── hardware.rs         # BIOS, motherboard, chassis, UUID
│   │       │   ├── cpu.rs              # CPUs, cores, threads, cache
│   │       │   ├── memory.rs           # RAM modules (DMI type 17)
│   │       │   ├── storage.rs          # Disks, optical, SMART (smartctl)
│   │       │   ├── video.rs            # Video cards / GPUs
│   │       │   ├── sound.rs            # Sound cards
│   │       │   ├── battery.rs          # Batteries (laptops)
│   │       │   ├── screen.rs           # Monitors via EDID (edid.ids)
│   │       │   ├── usb.rs              # USB devices (usb.ids lookup)
│   │       │   ├── pci.rs              # PCI/PCIe devices (pci.ids lookup)
│   │       │   │
│   │       │   # Software & system
│   │       │   ├── os.rs               # OS name/version, kernel, arch, HostID,
│   │       │   │                       #   install date
│   │       │   ├── software.rs         # Packages, updates, patches, UWP (Windows)
│   │       │   ├── processes.rs        # Processes (namespace-aware; Win32 since 1.17)
│   │       │   ├── users.rs            # Users, lastLoggedUser (FQDN-aware, last -w)
│   │       │   ├── timezone.rs         # System timezone
│   │       │   ├── environment.rs      # Environment variables
│   │       │   │
│   │       │   # Network
│   │       │   ├── network.rs          # Interfaces, IP, MAC, WiFi, IPv6, speed
│   │       │   ├── printers.rs         # Local printers (CUPS, WMI; portname dedup)
│   │       │   │
│   │       │   # Security
│   │       │   ├── antivirus/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── windows.rs      # WMI SecurityCenter2 + service detection:
│   │       │   │   │                   #   Defender, McAfee/Trellix, Kaspersky,
│   │       │   │   │                   #   ESET, Avira, Bitdefender, Norton,
│   │       │   │   │                   #   CrowdStrike, Cortex XDR, SentinelOne,
│   │       │   │   │                   #   F-Secure, Trend Micro, WithSecure
│   │       │   │   ├── linux.rs        # CrowdStrike, DrWeb, Kaspersky,
│   │       │   │   │                   #   ESET, SentinelOne, Cortex XDR
│   │       │   │   └── macos.rs        # CrowdStrike, WithSecure
│   │       │   │
│   │       │   # Remote management agents
│   │       │   ├── remote_mgmt/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── anydesk.rs
│   │       │   │   ├── meshcentral.rs
│   │       │   │   ├── litemanager.rs
│   │       │   │   ├── simplehelp.rs
│   │       │   │   ├── rudesktop.rs
│   │       │   │   ├── teamviewer.rs
│   │       │   │   ├── tacticalrmm.rs
│   │       │   │   └── rustdesk.rs
│   │       │   │
│   │       │   # Databases
│   │       │   ├── databases/
│   │       │   │   ├── mod.rs          # DB inventory: version, port, instances;
│   │       │   │   │                   #   credentials supplied via config
│   │       │   │   ├── mysql.rs        # MySQL / MariaDB
│   │       │   │   ├── postgresql.rs   # PostgreSQL (multi-instance; Linux + Windows)
│   │       │   │   ├── oracle.rs       # Oracle
│   │       │   │   ├── mssql.rs        # Microsoft SQL Server (multi-instance)
│   │       │   │   └── mongodb.rs      # MongoDB
│   │       │   │
│   │       │   # Virtualization
│   │       │   ├── virtualization/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── virtualbox.rs   # VBoxManage CLI
│   │       │   │   ├── parallels.rs    # Parallels Desktop (macOS)
│   │       │   │   ├── virtuozzo.rs    # Virtuozzo / OpenVZ
│   │       │   │   ├── jails.rs        # BSD jails (FreeBSD)
│   │       │   │   ├── kvm.rs          # KVM + QEMU (libvirt / virsh, vCPU)
│   │       │   │   ├── docker.rs       # Docker (cgroup v2 aware)
│   │       │   │   ├── lxd.rs          # LXD (full-path check)
│   │       │   │   ├── lxc.rs          # LXC / Proxmox LXC (memory conversion)
│   │       │   │   ├── hyperv.rs       # Hyper-V (WMI)
│   │       │   │   ├── wsl.rs          # WSL (must NOT be classified as Hyper-V)
│   │       │   │   ├── proxmox.rs      # Proxmox VE
│   │       │   │   ├── vmware_desktop.rs  # VMware Workstation / Desktop
│   │       │   │   └── solaris_zones.rs   # Solaris zones (zonecfg / zoneadm)
│   │       │   │
│   │       │   # Management hardware
│   │       │   └── ipmi/
│   │       │       ├── mod.rs
│   │       │       ├── ilo.rs          # HP iLO (IP resolution: Linux + Windows)
│   │       │       └── fru.rs          # IPMI FRU data
│   │       │
│   │       ├── platform/               # ══ Platform implementations ══
│   │       │   ├── linux/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── proc.rs         # /proc, /sys, sysfs
│   │       │   │   ├── dmidecode.rs    # dmidecode parser (BIOS, RAM, CPU)
│   │       │   │   ├── lspci.rs        # lspci output
│   │       │   │   ├── lsusb.rs        # lsusb output
│   │       │   │   ├── distro.rs       # Distro detection (incl. Astra Linux)
│   │       │   │   └── rhn.rs          # Red Hat Network SystemID
│   │       │   ├── windows/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── wmi_worker.rs   # Dedicated COM/WMI worker thread (see §0.3):
│   │       │   │   │                   #   one OS thread, CoInitializeEx once,
│   │       │   │   │                   #   mpsc channel API; never Send COM across
│   │       │   │   │                   #   Tokio tasks
│   │       │   │   ├── wmi.rs          # High-level WMI query API over the worker
│   │       │   │   ├── registry.rs     # Read, wildcards, 32/64-bit view,
│   │       │   │   │                   #   REG_MULTI_SZ, UTF-16LE decoding
│   │       │   │   ├── powershell.rs   # PowerShell exec (UTF-16LE, quoting-safe)
│   │       │   │   ├── keystore.rs     # Windows certificate store / CNG
│   │       │   │   ├── codepage.rs     # System codepage, UTF-16LE → UTF-8
│   │       │   │   ├── vpn.rs          # VPN / virtual adapter detection (reg + WMI)
│   │       │   │   ├── services.rs     # Windows services (for AV detection)
│   │       │   │   ├── uwp.rs          # Windows Store / UWP packages
│   │       │   │   ├── azure_ad.rs     # Azure AD users (scan-profiles)
│   │       │   │   └── arch.rs         # 32/64-bit detection (is64bit)
│   │       │   ├── macos/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── system_profiler.rs
│   │       │   │   ├── ioreg.rs        # IOKit / ioreg (avoid deep-recursion bug)
│   │       │   │   ├── networksetup.rs
│   │       │   │   └── keychain.rs     # macOS Keychain (system-ssl-ca)
│   │       │   ├── solaris/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── smbios.rs       # smbios parsing: memory, UUID
│   │       │   │   ├── cpu.rs          # OmniOS CPU inventory
│   │       │   │   └── zones.rs        # Solaris / OmniOS zones
│   │       │   ├── hpux/
│   │       │   │   ├── mod.rs
│   │       │   │   └── uptime.rs       # HP-UX uptime + base inventory
│   │       │   ├── aix/
│   │       │   │   └── mod.rs          # AIX base inventory
│   │       │   └── freebsd/
│   │       │       ├── mod.rs
│   │       │       └── storage.rs      # FreeBSD storage inventory
│   │       │
│   │       └── assets/                 # ══ Embedded data files ══
│   │           ├── edid.ids            # Monitor vendor IDs (EDID)
│   │           ├── pci.ids             # PCI vendor/device IDs
│   │           └── usb.ids             # USB vendor/device IDs
│   │           # (sysobject.ids lives in glpi-discovery)
│   │
│   # ───────────────────────────────────────────────────
│   # C — NETWORK DISCOVERY
│   # ───────────────────────────────────────────────────
│   ├── glpi-discovery/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs               # Traits: DiscoveryMethod, MibSupport, NetTask
│   │       ├── scanner.rs              # Tokio parallel scanner + Semaphore + timeout
│   │       ├── ip_range.rs             # CIDR, range, single-IP iterator
│   │       │
│   │       ├── methods/                # ══ Discovery methods ══
│   │       │   ├── ping.rs             # ICMP (see §0.2: ping-rs / DGRAM + TCP fallback)
│   │       │   ├── arp.rs              # ARP table lookup (OS API)
│   │       │   ├── netbios.rs          # UDP port 137 (NetBIOS name query)
│   │       │   └── snmp/
│   │       │       ├── mod.rs
│   │       │       ├── transport.rs    # tokio UdpSocket transport
│   │       │       ├── codec.rs        # rasn + rasn-snmp encode/decode
│   │       │       ├── v1v2c.rs        # community-string based
│   │       │       ├── v3.rs           # USM engine: auth MD5/SHA-1/224/256/384/512
│   │       │       │                   #             priv DES/AES128/192/256/192C/256C
│   │       │       │                   #   (built in-crate; see §0.1)
│   │       │       ├── v3_context.rs   # SNMPv3 contextname field (1.17)
│   │       │       ├── retries.rs      # snmp-retries config
│   │       │       ├── advanced.rs     # snmp-advanced-support.cfg handling
│   │       │       ├── sysobject.rs    # sysobject.ids parser: OID → type/vendor/model
│   │       │       │                   #   + sysObjectID-as-string handling
│   │       │       └── mib/            # ══ MIB support modules ══
│   │       │           ├── mod.rs      # MibSupport trait + runtime registry
│   │       │           │
│   │       │           # Standard MIBs (always active)
│   │       │           ├── system_mib.rs       # sysDescr, sysObjectID, sysName
│   │       │           ├── if_mib.rs           # ifTable, ifXTable
│   │       │           ├── entity_mib.rs       # CPUs, RAM, modules
│   │       │           ├── printer_mib.rs      # RFC 3805: counters, cartridges
│   │       │           ├── bridge_mib.rs       # MAC table, spanning tree
│   │       │           ├── lldp.rs             # LLDP neighbor discovery
│   │       │           ├── cdp.rs              # Cisco Discovery Protocol
│   │       │           ├── ip_mib.rs           # IP-MIB fallback for port IPs
│   │       │           │
│   │       │           # Network switches / routers
│   │       │           ├── cisco.rs            # Cisco IOS, NX-OS
│   │       │           ├── cisco_ucs.rs        # Cisco UCS board
│   │       │           ├── juniper.rs          # Juniper (stacking, connections)
│   │       │           ├── fortinet.rs         # FortiGate, HA passive element
│   │       │           ├── mikrotik.rs         # Mikrotik RouterOS
│   │       │           ├── nokia.rs            # Nokia
│   │       │           ├── d_link.rs           # D-Link
│   │       │           ├── foxgate.rs          # FoxGate
│   │       │           ├── tiesse.rs           # Tiesse
│   │       │           ├── extreme_networks.rs # Extreme Networks (VLANs)
│   │       │           ├── netgear.rs          # Netgear (stacking)
│   │       │           ├── aruba.rs            # Aruba (SSID)
│   │       │           ├── aerohive.rs         # Aerohive WLAN
│   │       │           ├── watchguard.rs       # WatchGuard firewalls
│   │       │           ├── sophos.rs           # Sophos (serial number fix)
│   │       │           ├── telco_systems.rs    # Telco Systems T-Mark
│   │       │           ├── intelbras.rs        # Intelbras
│   │       │           │
│   │       │           # Load balancers
│   │       │           ├── netscaler.rs        # Citrix NetScaler
│   │       │           ├── radware.rs          # Radware Alteon
│   │       │           │
│   │       │           # Storage
│   │       │           ├── hitachi_vantara.rs  # Hitachi Vantara
│   │       │           ├── quantum.rs          # Quantum storage library
│   │       │           ├── veritas.rs          # Veritas NetBackup appliance
│   │       │           ├── dell_emc.rs         # Dell EMC (experimental OIDs)
│   │       │           │
│   │       │           # Linux appliances
│   │       │           ├── linux_appliance.rs  # Synology, Ubiquiti, Katusha
│   │       │           ├── tp_link.rs          # TP-Link + Omada
│   │       │           │
│   │       │           # PDUs / UPS
│   │       │           ├── raritan.rs          # Raritan PDU (plugs; GLPI 12)
│   │       │           ├── eaton.rs            # Eaton PDU
│   │       │           ├── socomec.rs          # Socomec PDU
│   │       │           ├── bachmann.rs         # Bachmann PDU
│   │       │           ├── rnx.rs              # RNX PDU (plugs)
│   │       │           ├── digipower.rs        # Digipower PDU
│   │       │           ├── riello.rs           # Riello UPS
│   │       │           │
│   │       │           # Printers (SNMP inventory)
│   │       │           ├── hp_printer.rs       # HP printers + storage (private OID)
│   │       │           ├── brother.rs          # Brother
│   │       │           ├── ricoh.rs            # Ricoh (page + scan counters)
│   │       │           ├── canon.rs            # Canon (page counters, LPB7660)
│   │       │           ├── xerox.rs            # Xerox (page counters)
│   │       │           ├── konica.rs           # Konica (page counters, firmware)
│   │       │           ├── lexmark.rs          # Lexmark
│   │       │           ├── pantum.rs           # Pantum
│   │       │           ├── sindoh.rs           # Sindoh
│   │       │           ├── epson.rs            # Epson (maintenance cartridge)
│   │       │           │
│   │       │           # KVM / monitoring
│   │       │           ├── avocent.rs          # Avocent KVM
│   │       │           ├── hikvision.rs        # Hikvision cameras
│   │       │           │
│   │       │           # Telephony / VoIP
│   │       │           ├── avaya.rs            # Avaya J100 IP phones
│   │       │           ├── htek.rs             # HTek phones
│   │       │           └── snom.rs             # Snom phones (advanced-support.cfg)
│   │       │
│   │       └── tasks/
│   │           ├── net_discovery.rs    # NetDiscovery task
│   │           └── net_inventory.rs    # NetInventory task (incl. IEC 61850 merge)
│   │
│   ├── glpi-iec61850/                  # IEC 61850 / OT / IED (optional feature)
│   │   ├── build.rs                    # bindgen for libiec61850
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ffi.rs                  # libiec61850 v1.6.x C FFI (static link)
│   │       ├── discovery.rs            # IED discovery (impl DiscoveryMethod)
│   │       ├── inventory.rs            # IED inventory data
│   │       └── merge.rs                # Merge IEC 61850 + SNMP (1.17)
│   │
│   # ───────────────────────────────────────────────────
│   # D — REMOTE INVENTORY
│   # ───────────────────────────────────────────────────
│   ├── glpi-inventory-remote/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── task.rs                 # RemoteInventory task
│   │       ├── workers.rs              # remote-workers parallelism
│   │       ├── state.rs                # Checksums / state files (delta inventory);
│   │       │                           #   maintenance: clean up entries > 30 days
│   │       ├── ssh/
│   │       │   ├── mod.rs
│   │       │   ├── mode_cli.rs         # Mode 1: SSH command-line tool
│   │       │   ├── mode_native.rs      # Mode 2: russh (libssh2 replacement)
│   │       │   ├── mode_perl.rs        # Mode 3: Perl on remote system
│   │       │   ├── executor.rs         # Remote command exec + file read
│   │       │   ├── known_hosts.rs      # known_hosts (Windows path fix)
│   │       │   └── options.rs          # assetname-support (1/2/3), fqdn, itemtype
│   │       └── winrm/
│   │           ├── mod.rs
│   │           ├── client.rs           # WinRM protocol
│   │           ├── powershell.rs       # Remote PowerShell (UTF-16LE)
│   │           ├── wmi.rs              # Remote WMI (WsMan; SessionID fix)
│   │           ├── registry.rs         # Remote registry
│   │           └── options.rs          # battery, timezone, itemtype
│   │
│   ├── glpi-vsphere/                   # VMware ESX / vCenter
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── task.rs                 # ESX task (impl Task trait)
│   │       ├── client.rs               # HTTPS SOAP client (reqwest + quick-xml)
│   │       ├── soap/
│   │       │   ├── session.rs          # Login / logout, session timeout
│   │       │   ├── host.rs             # HostSystem: CPU, RAM, BIOS (filter invalid)
│   │       │   ├── vm.rs               # VirtualMachine: OS, IP, vCPU
│   │       │   │                       #   (GLPI 10.0.17+ schema v1.1.36)
│   │       │   └── datacenter.rs       # vCenter: datacenter / cluster
│   │       ├── inventory.rs            # → GLPI device format
│   │       └── options.rs              # esx-itemtype, glpi-version, dump/dumpfile
│   │
│   # ───────────────────────────────────────────────────
│   # E — AGENT TASKS & DAEMON
│   # ───────────────────────────────────────────────────
│   ├── glpi-collect/                   # Collect task v3.0
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── task.rs
│   │       ├── registry.rs             # Registry keys (REG_MULTI_SZ, all types)
│   │       ├── wmi.rs                  # WMI queries (server-driven; uses wmi_worker)
│   │       ├── file.rs                 # Read file contents
│   │       └── checksum.rs             # SHA-256 (checkSumSHA256) + SHA-512
│   │
│   ├── glpi-deploy/                    # Deploy task v3.5
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── task.rs
│   │       ├── downloader.rs           # HTTP download (GLPI server)
│   │       ├── p2p.rs                  # P2P mirror (httpd-port, remote-workers;
│   │       │                           #   never scan network/broadcast addresses)
│   │       ├── checksum.rs             # SHA-512: FileSHA512 + FileSHA512Mismatch
│   │       ├── executor.rs             # Run installers/scripts; PowerShell (Windows)
│   │       ├── checks.rs               # Return-code checks, output matching
│   │       └── reporter.rs            # Status reporting + partial software inventory
│   │
│   ├── glpi-wakeonlan/                 # WakeOnLan task
│   │   └── src/
│   │       ├── lib.rs
│   │       └── magic_packet.rs         # 102-byte magic packet, UDP broadcast port 9
│   │
│   ├── glpi-scheduler/                 # Daemon scheduling
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── scheduler.rs            # nextRunDate + jitter + progressive backoff
│   │       │                           #   on network errors (60s, doubling)
│   │       ├── target.rs               # ServerTarget, LocalTarget
│   │       ├── event.rs                # Events: init, runnow, taskrun,
│   │       │                           #   partial, maintenance, job
│   │       ├── fork.rs                 # Unix: fork() + IPC pipe
│   │       │                           # Windows: CreateProcess + named pipe
│   │       └── ipc.rs                  # IPC protocol (long messages, SSL rename)
│   │
│   ├── glpi-http/                      # Embedded HTTP server
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs               # axum, port 62354, httpd-ip, httpd-trust
│   │       ├── api/
│   │       │   ├── status.rs           # GET /status (plain text, for GLPI server)
│   │       │   ├── now.rs              # GET|POST /now
│   │       │   │                       #   ?partial=yes|no  ?full=yes|no
│   │       │   │                       #   ?task=all|task1,task2  ?delay=N
│   │       │   └── index.rs            # GET / (targets + tasks for trusted IPs)
│   │       └── toolbox/                # ToolBox plugin v1.7
│   │           ├── mod.rs
│   │           ├── credentials.rs      # SNMP, SSH, WinRM, VMware credentials
│   │           ├── ip_range.rs         # IP ranges
│   │           ├── scheduling.rs       # Scheduling
│   │           ├── mib_support.rs      # MIB module overview
│   │           ├── remotes.rs          # Remote targets
│   │           ├── results.rs          # Results (tag_filter)
│   │           ├── inventory.rs        # Inventory configuration
│   │           ├── configuration.rs    # Agent configuration
│   │           └── yaml_export.rs      # YAML export
│   │
│   ├── glpi-plugins/                   # HTTP server plugins
│   │   └── src/
│   │       ├── lib.rs                  # HttpServerPlugin trait
│   │       ├── proxy.rs                # Proxy plugin v3.0
│   │       │                           #   (NetDiscovery/NetInventory forwarding)
│   │       └── ssl.rs                  # SSL plugin v2.0 (Windows + Unix)
│   │
│   └── glpi-cli/                       # CLI binary (all commands)
│       └── src/
│           ├── main.rs
│           └── cmd/
│               ├── inventory.rs        # glpi-agent inventory
│               ├── netdiscovery.rs     # glpi-agent netdiscovery
│               ├── netinventory.rs     # glpi-agent netinventory
│               ├── esx.rs              # glpi-agent esx
│               ├── remoteinventory.rs  # glpi-agent remoteinventory
│               ├── injector.rs         # glpi-agent inject
│               ├── wakeup.rs           # glpi-agent wakeup
│               └── daemon.rs           # glpi-agent daemon
```

---

## 3. Inventory categories by platform

| Category | Linux | Windows | macOS | Solaris | HP-UX | AIX | FreeBSD |
|---|---|---|---|---|---|---|---|
| Hardware / BIOS | dmidecode, /sys | WMI | system_profiler | smbios | – | – | dmidecode |
| CPU | /proc/cpuinfo | WMI | system_profiler | smbios / OmniOS | – | – | – |
| RAM | dmidecode type 17 | WMI | system_profiler | smbios | – | – | – |
| Storage + SMART | lsblk, smartctl | WMI | diskutil | – | – | – | yes |
| Network + WiFi | ip, iwconfig | WMI, MSFT_NetAdapter | ifconfig, networksetup | – | – | – | – |
| VPN adapters | – | registry + WMI | – | – | – | – | – |
| OS | /etc/os-release, uname | WMI | sw_vers | uname | uname | uname | uname |
| Software | dpkg, rpm, flatpak | registry, MSI, UWP | pkgutil, Homebrew | – | – | – | pkg |
| Monitors / EDID | xrandr + edid.ids | WMI + edid.ids | system_profiler | – | – | – | – |
| Video / GPU | lspci | WMI | system_profiler | – | – | – | – |
| Sound | /proc/asound | WMI | system_profiler | – | – | – | – |
| Batteries | /sys/class/power | WMI | system_profiler | – | – | – | – |
| USB | lsusb + usb.ids | WMI + usb.ids | system_profiler | – | – | – | – |
| PCI | lspci + pci.ids | WMI + pci.ids | – | – | – | – | – |
| Printers | CUPS | WMI (portname dedup) | system_profiler | – | – | – | – |
| Processes | /proc (ns-aware) | WMI | ps | – | – | – | – |
| Users | last -w (FQDN) | WMI | last | – | – | – | – |
| Timezone | /etc/localtime | WMI / registry | systemsetup | – | – | – | – |
| AntiVirus | CrowdStrike, DrWeb, ESET, Kaspersky, SentinelOne, Cortex XDR | WMI SecurityCenter2 + services | CrowdStrike, WithSecure | – | – | – | – |
| Remote mgmt | AnyDesk, MeshCentral, TacticalRMM, TeamViewer, RustDesk | LiteManager, SimpleHelp, RuDesktop, AnyDesk | – | – | – | – | – |
| Databases | MySQL, PostgreSQL, Oracle | MSSQL, MySQL, PostgreSQL | – | – | – | – | – |
| IPMI / iLO | IpmiTools, iLO IP | iLO API (Windows) | – | – | – | – | – |
| Virtualization | KVM, Docker, LXD, LXC, Proxmox, VBox, WSL | Hyper-V, QEMU, VMware Desktop, WSL | Parallels, VBox | Zones | – | – | Jails |

---

## 4. MIB modules (Network Discovery / Inventory)

### Standard MIBs (always active)
| Module | Function |
|---|---|
| `system_mib` | sysDescr, sysObjectID, sysName, sysLocation |
| `if_mib` | ifTable, ifXTable (network interfaces) |
| `entity_mib` | Entity-MIB: CPUs, RAM, hardware modules |
| `printer_mib` | RFC 3805: page counters, cartridges, trays |
| `bridge_mib` | Bridge-MIB: MAC table, spanning tree |
| `lldp` | LLDP neighbor discovery (connections) |
| `cdp` | Cisco Discovery Protocol |
| `ip_mib` | IP-MIB fallback for port-IP detection |

### Vendor MIBs

| Group | Modules |
|---|---|
| **Network** | cisco, cisco_ucs, juniper, fortinet, mikrotik, nokia, d_link, foxgate, tiesse, extreme_networks, netgear, aruba, aerohive, watchguard, sophos, telco_systems, intelbras |
| **Load balancers** | netscaler (Citrix), radware (Alteon) |
| **Storage** | hitachi_vantara, quantum, veritas, dell_emc |
| **Linux appliances** | linux_appliance (Synology, Ubiquiti, Katusha), tp_link (+ Omada) |
| **PDUs / UPS** | raritan, eaton, socomec, bachmann, rnx, digipower, riello |
| **Printers** | hp_printer, brother, ricoh, canon, xerox, konica, lexmark, pantum, sindoh, epson |
| **KVM / camera** | avocent, hikvision |
| **Telephony** | avaya, htek, snom |

**Total: 8 standard MIBs + 35 vendor MIBs = 43 MIB modules**

---

## 5. Configuration options (complete)

| Option | Description |
|---|---|
| **Connection** | |
| `server` | GLPI server URL(s) |
| `local` | Local output directory |
| `proxy` | HTTP proxy (`none` = disable) |
| **Tasks** | |
| `tasks` | Tasks to run (comma-separated) |
| `no-task` | Tasks to disable |
| **Scheduling** | |
| `delaytime` | Max delay before first run (seconds) |
| `lazy` | Do not run before nextRunDate |
| `conf-reload-interval` | Config reload interval (0 = off) |
| **Inventory** | |
| `no-category` | Exclude inventory categories |
| `required-category` | Always include in partial inventory |
| `scan-homedirs` | Scan software in home dirs |
| `scan-profiles` | Scan software in user profiles (Azure AD) |
| `full-inventory-postpone` | Delta inventory: partial for N days, then full |
| `additional-content` | Merge additional JSON content |
| `assetname-support` | Hostname format: 1 = short, 2 = as-is, 3 = fqdn |
| `itemtype` | Asset type (GLPI 11+ genericity) |
| `glpi-version` | Target GLPI version for format features |
| **SNMP** | |
| `snmp-retries` | SNMP retry count (default 0) |
| `snmp-advanced-support.cfg` | Edge-device config (Snom, etc.) |
| **SSL / auth** | |
| `ssl-cert-file` | Client certificate |
| `ssl-key-file` | Private key (separate file) |
| `ssl-fingerprint` | Server certificate fingerprint |
| `ca-cert-file` | CA certificate file |
| `ca-cert-dir` | CA certificate directory |
| `ssl-keystore` | `system-ssl-ca` (macOS Keychain) |
| `no-ssl-check` | Disable SSL validation |
| **HTTP server** | |
| `no-httpd` | Disable HTTP server |
| `httpd-ip` | Listen interface (default: all) |
| `httpd-port` | Port (default 62354) |
| `httpd-trust` | Trusted IPs / ranges |
| **Remote** | |
| `remote` | Remote targets (`ssh://`, `winrm://`) |
| `remote-workers` | Parallel remote connections |
| **ESX** | |
| `esx-itemtype` | VMware asset type (GLPI 11+) |

---

## 6. Phase plan

### Phase 1 — Foundation (months 1–2)
Crates: `glpi-core`, `glpi-transport`

- Cargo workspace, CI/CD (GitHub Actions), cross-compile matrix
- All types: Device, NetworkInterface, SnmpCredentials (incl. contextname)
- Config: TOML + CLI + Windows registry + conf.d/*.cfg with explicit precedence (§0.4)
- All options from §5
- GLPI native JSON protocol (START / DEVICE / STOP)
- FusionInventory XML compatibility protocol
- Partial-inventory logic (full-inventory-postpone, required-category)
- Auth: Basic, OAuth2 (incl. /front/inventory.php case)
- SSL: Windows store (CNG), macOS Keychain, fingerprint, ca-cert-dir
- Logging backends: stderr, file, syslog; callback API
- HTTP transport: reqwest, TLS, compression, retry
- glpi-injector: all SSL options + OAuth2 + agentid
- Feature gates: glpi-version-dependent format features
- **Golden-file test harness vs. Perl agent (§0.5, §13) — build this now**
- **Tests:** port `t/agent/config*.t`, `t/agent/http/*`, protocol/serialization tests; set up the fixture-import pipeline (§13); CI fails if coverage drops below threshold

### Phase 2 — NetDiscovery core (months 2–4)
Crate: `glpi-discovery` (scanner + methods + SNMP stack)

- IP-range iterator (CIDR, range, single addresses)
- Ping: dual strategy (DGRAM/`ping-rs` + TCP fallback), document CAP_NET_RAW (§0.2)
- ARP table lookup (OS API, cross-platform)
- NetBIOS UDP port 137 (name query)
- **SNMP stack assembly (§0.1):** UDP transport, rasn codec, v1/v2c
- **SNMPv3 USM in-crate:** auth MD5/SHA-1/224/256/384/512; priv DES/AES128/192/256/192C/256C
- SNMPv3 contextname field (1.17)
- snmp-retries config
- snmp-advanced-support.cfg parser + edge-device handling
- sysobject.ids parser: OID → type/vendor/model; sysObjectID-as-string
- Tokio parallel scanner: Semaphore, timeout, progress tracking
- NetDiscovery task (full; IEC 61850 hook prepared)
- **Tests:** migrate `t/agent/snmp/*` (incl. `mock.t`), reuse all `resources/walks/*.walk` and `*.result` SNMP-walk fixtures as Rust test data; verify v1/v2c/v3 decode against captured packets; USM crypto vectors (RFC 3414/7860 test vectors) for every auth/priv algorithm

### Phase 3 — NetInventory + all MIB modules (months 3–7)
Crate: `glpi-discovery` (MIBs + inventory task)

- NetInventory task skeleton
- All 8 standard MIBs (system, if, entity, printer, bridge, lldp, cdp, ip_mib)
- PDU type support (GLPI 12 preview)
- All 35 vendor MIB modules (see §4)
- Runtime MIB registry (extensible via trait)
- Automatic sysobject.ids update logic
- **Tests:** each MIB module ships with its SNMP-walk fixture and an expected-device assertion, migrated from the Perl `t/` device cases; a new vendor MIB is not merged without its walk fixture + golden output

### Phase 4 — IEC 61850 (months 5–7, parallel to Phase 3)
Crate: `glpi-iec61850`

- libiec61850 v1.6.x C FFI via bindgen; static link
- IED discovery (impl DiscoveryMethod)
- Retrieve IED inventory data
- Merge IEC 61850 + SNMP in NetInventory (1.17)
- `iec61850` feature flag (not a mandatory link)
- ToolBox UI page for IEC 61850 config
- **Tests:** mock IED responses; verify SNMP+IEC61850 merge output matches Perl golden files

### Phase 5 — CLI + daemon + HTTP (months 5–8)
Crates: `glpi-scheduler`, `glpi-http`, `glpi-plugins`, `glpi-cli`

- All CLI commands with full options
- Scheduling: nextRunDate, jitter, progressive backoff (60s × 2)
- Foreground mode + daemon mode (continuous)
- Event system: init, runnow, taskrun, partial, maintenance, job
- Task forking — Unix: fork() + IPC pipe; Windows: CreateProcess + named pipe
- IPC protocol (long messages, SSL-rename events)
- conf-reload-interval (notify crate)
- HTTP server: /status, / (targets for trusted IPs)
- /now with full query-string API (partial, full, task, delay)
- httpd-trust IP filtering, httpd-ip, no-httpd
- ToolBox v1.7: all 9 pages incl. config + YAML export
- Proxy plugin v3.0 (NetDiscovery/NetInventory forwarding)
- SSL plugin v2.0 (Windows support)
- **Tests:** migrate scheduler/target/event tests; HTTP API tests (status, /now query parsing, httpd-trust enforcement); IPC round-trip tests incl. long-message handling

### Phase 6 — Local inventory (months 7–11)
Crate: `glpi-inventory-local`

**6a — All inventory categories**
- High priority: hardware/BIOS, CPU, RAM, storage, OS, software, network, AntiVirus, remote-mgmt, virtualization
- Medium: printers, monitors/EDID, USB, PCI, video, sound, batteries, processes, users, timezone, environment, databases
- Low: IPMI / HP iLO

**6b — Windows implementation details**
- COM/WMI worker thread (§0.3) — single OS thread, channel API
- Registry: wildcards, 32/64-bit view, REG_MULTI_SZ, UTF-16LE
- Windows certificate store / CNG
- Codepage handling (UTF-16LE → UTF-8)
- VPN / virtual adapter detection (registry + WMI)
- Windows Store / UWP packages (skip nameless packages)
- Azure AD users (scan-profiles)
- Windows services (for AV detection)
- 32/64-bit architecture detection

**6c — Exotic platforms**
- Solaris / OmniOS: smbios (memory, UUID), OmniOS CPU, zones
- HP-UX: base inventory (uptime, hardware)
- AIX: base inventory
- FreeBSD: storage inventory, jails
- All Linux: distro detection (incl. Astra Linux), RHN SystemID

**6d — Tests (largest test-migration block)**
- This phase carries the bulk of the Perl suite: `t/tasks/inventory/{generic,linux,win32,macos,hpux,aix,solaris}/**`
- Each category module migrates with its fixtures: `resources/generic/dmidecode/*`, `resources/linux/{iwconfig,smartctl,...}/*`, `resources/macos/{ifconfig,ioreg}/*`, `resources/win32/wmi/*.wmi`, `resources/generic/edid/*`, `resources/virtualization/**`
- Parser tests run on every captured command output; a category module is "done" only when its migrated sub-tests pass
- Windows WMI/registry tests run against captured `.wmi` dumps so they execute on Linux CI without a Windows host

### Phase 7 — Remote inventory (months 9–11)
Crate: `glpi-inventory-remote`

- SSH mode 1: command-line tool
- SSH mode 2: russh (libssh2 replacement)
- SSH mode 3: Perl on remote system
- known_hosts handling (Windows path fix)
- assetname-support option (1/2/3) for SSH
- WinRM: protocol + PowerShell + remote WMI
- WinRM: remote registry, SessionID WsMan fix
- remote-workers parallelism
- State files / checksums (delta-inventory diff)
- Maintenance: clean up state files > 30 days
- itemtype option (GLPI 11+)
- HP-UX + UnixWare timezone support
- **Tests:** migrate `t/tasks/remoteinventory.t`; mock SSH/WinRM sessions with captured command transcripts; verify delta-inventory state-file logic

### Phase 8 — vSphere / ESX (months 9–11, parallel to Phase 7)
Crate: `glpi-vsphere`

- HTTPS SOAP client (reqwest + quick-xml)
- Session management (login/logout, session timeout)
- ESXi direct: host + VMs
- vCenter: datacenter / cluster
- VM OS + IP reporting (GLPI 10.0.17+ schema v1.1.36)
- BIOS filter (drop invalid values)
- Total-RAM estimate as memory component
- esx-itemtype + glpi-version options
- --dump + --dumpfile mode
- **Tests:** reuse `*-hostfullinfo.dump` ESX fixtures; `--dumpfile` lets tests run fully offline; golden JSON comparison vs. Perl ESX task

### Phase 9 — Collect + Deploy + WakeOnLan (months 10–12)
Crates: `glpi-collect`, `glpi-deploy`, `glpi-wakeonlan`

- **Collect v3.0:** registry (all types incl. REG_MULTI_SZ), WMI, file read
- Collect: SHA-256 (checkSumSHA256) + SHA-512, CSRF handling
- **Deploy v3.5:** HTTP download, P2P mirror (no scan of broadcast addresses)
- Deploy: SHA-512 (FileSHA512 + FileSHA512Mismatch, case-insensitive)
- Deploy: run installers/scripts, PowerShell (quoting-safe)
- Deploy: return-code checks, output matching
- Deploy: partial software inventory after successful deployment
- **WakeOnLan:** magic packet (102 bytes), UDP broadcast port 9
- **Tests:** migrate `t/tasks/deploy/**` (incl. CheckProcessor cases FileSHA512 / FileSHA512Mismatch), Collect file/registry checks, WakeOnLan packet-construction byte assertion

### Phase 10 — Stabilization + packaging (months 12–15)

- Integration tests: mock SNMP, mock GLPI server, mock vSphere
- JSON/XML schema parity: automated diff tests vs. Perl agent (§0.5)
- **Test-suite parity audit:** confirm every migrated Perl test file has a Rust counterpart; produce a coverage map (Perl test file → Rust test) and document any intentionally dropped cases with justification
- Performance tests: scan speed, RAM usage
- Cross-compile: Windows x86_64/ARM64, Linux x86_64/ARM64, macOS x86_64/ARM64
- Windows: MSI (WiX) + GLPI-AgentMonitor integration, CVE hardening
- Linux: DEB, RPM, Snap, AppImage (incl. libiec61850 packages)
- macOS: PKG
- libiec61850: static library for all platforms
- Security audit, CVE review
- Documentation: rustdoc, man pages, migration guide

---

## 7. Timeline

```
Month   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15

P1  Foundation (Core, Transport)
    [=======]
P2  NetDiscovery core + SNMP stack
            [===========]
P3  NetInventory + 43 MIB modules
                [===================]
P4  IEC 61850 (parallel to P3)
                    [===========]
P5  CLI + daemon + HTTP
                        [===========]
P6  Local inventory (22 categories + Win + exotic platforms)
                                [===================]
P7  Remote inventory
                                            [===========]
P8  vSphere / ESX (parallel to P7)
                                            [===========]
P9  Collect + Deploy + WakeOnLan
                                                    [=======]
P10 Stabilization + packaging
                                                        [===========]
```

> Test migration is **not** a separate band: it runs continuously inside every phase (see the "Tests" line in each phase and §13). The fixture-import pipeline is built in P1; the test-parity audit closes it out in P10.

---

## 8. Dependencies

| Crate | Version | Category | Use |
|---|---|---|---|
| `tokio` | 1.x | Async | Runtime, concurrency |
| `async-trait` | 0.1 | Async | Async traits |
| `tokio-stream` | 0.1 | Async | Stream utilities |
| `futures` | 0.3 | Async | Future combinators |
| `reqwest` | 0.12 | HTTP | Client (TLS, compression, retry) |
| `axum` | 0.7 | HTTP | HTTP server (ToolBox) |
| `quick-xml` | 0.36 | XML | FusionInventory + vSphere SOAP |
| `serde`, `serde_json` | 1.x | Serde | Serialization |
| `snmp2` | 0.5 | SNMP | v1/v2c/v3 client, async (`tokio`), full auth/priv matrix incl. Cisco key extension (§0.1). Elect MIT license arm. |
| `socket2` | 0.5 | Discovery | DGRAM/raw ICMP socket control (ping, §0.2) |
| `clap` | 4.x | CLI | Argument parsing |
| `config` | 0.14 | Config | TOML + env base (registry/conf.d in-crate — §0.4) |
| `notify` | 6.x | Config | File-change watcher |
| `tracing`, `tracing-subscriber` | 0.1 | Logging | Structured logging + backends |
| `thiserror` | 1.x | Error | Error types |
| `anyhow` | 1.x | Error | Error handling (binaries) |
| `russh` | 0.43 | Remote | SSH (Remote Inventory) |
| `winreg` | 0.52 | Windows | Registry (cfg(windows)) |
| `wmi` | 0.13 | Windows | WMI queries — only via COM worker thread (§0.3) |
| `windows` | 0.58 | Windows | Win32 API, CNG, certificate store |
| `tokio-cron-scheduler` | 0.10 | Scheduler | Scheduling + jitter |
| `libiec61850-sys` (or in-tree bindgen) | 1.6 | FFI | IEC 61850 C library (optional) |
| `bindgen` | 0.70 | FFI | C FFI codegen (build-dep) |
| `sha2` | 0.10 | Crypto | SHA-256/512 (Deploy/Collect verification) |
| `mac_address` | 1.x | Network | MAC addresses (WakeOnLan) |

---

## 9. Feature flags

```toml
[features]
default = ["inventory", "netdiscovery", "netinventory", "wakeonlan", "http"]

# B — Local Inventory
inventory        = ["dep:glpi-inventory-local"]
databases        = ["inventory"]     # MySQL, PG, Oracle, MSSQL, MongoDB
ipmi             = ["inventory"]     # iLO / IPMI

# C — Network Discovery
netdiscovery     = ["dep:glpi-discovery"]
netinventory     = ["netdiscovery"]
iec61850         = ["netdiscovery", "dep:libiec61850-sys"]

# D — Remote Inventory
remote-inventory = ["dep:glpi-inventory-remote", "dep:russh"]
esx              = ["dep:glpi-vsphere", "dep:quick-xml"]

# E — Agent Tasks
collect          = ["dep:glpi-collect"]
deploy           = ["dep:glpi-deploy"]
wakeonlan        = ["dep:glpi-wakeonlan"]
http             = ["dep:glpi-http", "dep:axum"]
toolbox          = ["http"]
proxy-plugin     = ["http", "dep:glpi-plugins"]
ssl-plugin       = ["http", "dep:glpi-plugins"]

# Exotic platforms (set via target cfg at package build time)
platform-solaris = []
platform-hpux    = []
platform-aix     = []
platform-freebsd = []
```

---

## 10. Risks and mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| SNMPv3 USM crypto matrix (SHA-512 / AES256C) | ~~High~~ Low | **Resolved (§0.1):** `snmp2` provides the full auth/priv matrix incl. Cisco key extension (`KeyExtension::Reeder`). **Crypto-vector note:** `snmp2` 0.5 keeps password-to-key derivation and key localization private (only a generic `Hasher` is public), so RFC 3414/7860 vectors cannot be asserted against its internals through the public API. Crypto correctness is delegated to `snmp2`'s own test suite; the agent-side validation is a **live SNMPv3 round-trip** integration test (needs a v3 agent — deferred to Phase 10 / when a target is available). Residual: `snmp2` 0.5 cannot set a non-default `contextName`. |
| WMI COM apartment threading | High | Dedicated COM worker thread + mpsc channel; never Send COM across Tokio tasks (§0.3) |
| ICMP on Windows needs admin with raw sockets | Medium | Use `ping-rs` / DGRAM; TCP fallback; document CAP_NET_RAW |
| vSphere SOAP (no Rust SDK) | High | Minimal API calls; negotiate API version on connect |
| 43 MIB modules (correctness) | Medium | Reuse the Perl project's SNMP-walk fixtures as golden tests |
| libiec61850 FFI | Medium | bindgen; static link; CI on x86_64 first |
| Deploy P2P protocol | Medium | Reverse-engineer from Perl source; exclude network/broadcast addrs |
| Task forking on Windows | Medium | CreateProcess + named pipe; document IPC protocol |
| Solaris / HP-UX / AIX test environments | High | QEMU emulation or cloud VMs in CI |
| Windows codepage / UTF-16LE | Medium | Use Win32 API directly; test on non-Latin locales |
| JSON schema parity across 22+ categories | Medium | Automated diff tests vs. Perl output from day one (§0.5) |
| Config precedence + registry + conf.d not in `config` crate | Medium | Implement layering explicitly (§0.4) |
| Cross-compile ARM64 | Low | `cross` tool; GitHub Actions matrix |
| Test migration deferred / under-scoped | High | Tests migrate per-phase with their module (§13); CI gate blocks merge without passing migrated tests; Phase 10 parity audit catches gaps |
| Fixtures lost or not reused | Medium | Import `resources/**` verbatim in Phase 1 (§13 step 1); fixtures are version-controlled test data, never regenerated |

---

## 11. Acceptance criteria

- [ ] All 9 tasks implemented and tested (Inventory, NetDiscovery, NetInventory, ESX, RemoteInventory, Collect, Deploy, WakeOnLan, IEC 61850)
- [ ] All 22 inventory categories on Linux, Windows, macOS
- [ ] Solaris, HP-UX, AIX, FreeBSD: base inventory
- [ ] 43 MIB modules (8 standard + 35 vendor) implemented
- [ ] JSON output schema-compatible with GLPI Agent 1.17 (automated golden-file tests)
- [ ] FusionInventory XML format fully compatible
- [ ] GLPI 11+ genericity: itemtype, esx-itemtype
- [ ] Partial inventory + full-inventory-postpone + required-category
- [ ] All 35+ config options from §5 implemented
- [ ] SNMPv3 full algorithm matrix verified against device walks
- [ ] Scan performance ≥ 2× faster than Perl (NetDiscovery benchmark)
- [ ] Idle daemon memory < 50 MB
- [ ] Platforms: Windows x86_64/ARM64, Linux x86_64/ARM64, macOS x86_64/ARM64
- [ ] IEC 61850 as an optional feature (not a mandatory link)
- [ ] A new MIB module implementable in < 150 lines of Rust
- [ ] A new inventory category implementable in < 200 lines of Rust
- [ ] Test coverage > 80% for discovery core and NetInventory
- [ ] **Every Perl `t/` test file has a migrated Rust counterpart (parity map produced in Phase 10)**
- [ ] **All `resources/**` fixtures imported and reused as Rust test data (no regenerated/synthetic substitutes for real-device captures)**
- [ ] **Each module's migrated tests pass before that module is considered done (per-phase gate, not deferred)**
- [ ] **SNMPv3 crypto validated against published RFC 3414/7860 test vectors**
- [ ] **CI runs the full migrated suite on every PR and blocks merge on failure or coverage regression**

---

## 12. Suggested implementation order for Claude Code

Build in this dependency order so each step compiles and tests against the previous:

1. `glpi-core` types + error + config skeleton
2. Golden-file test harness + fixture import (capture Perl agent fixtures, §13)
3. `glpi-core` protocol (JSON + XML) + `glpi-transport`
4. `glpi-discovery` scanner + ping + arp + netbios
5. `glpi-discovery` SNMP stack (v1/v2c first, then v3 USM)
6. `glpi-discovery` standard MIBs, then vendor MIBs in batches
7. `glpi-cli` netdiscovery + netinventory commands (validate end-to-end early)
8. `glpi-scheduler` + `glpi-http` + daemon
9. `glpi-inventory-local` Linux first, then Windows (with COM worker), then macOS, then exotic
10. `glpi-inventory-remote` (SSH, then WinRM)
11. `glpi-vsphere`
12. `glpi-collect`, `glpi-deploy`, `glpi-wakeonlan`
13. `glpi-iec61850`
14. Packaging + CI matrix

For **every** crate above: write/migrate the corresponding tests in the same step. A crate is not "done" and the next step does not start until its migrated tests pass in CI. Step 2 (fixture import + harness) is a hard prerequisite for all later steps.

---

## 13. Test-migration strategy

The Perl agent ships roughly **200 test files and ~4300 sub-tests** under `t/`, plus a large `resources/` tree of real-world capture data. This suite is migrated as a first-class deliverable, in lockstep with the code it covers.

### 13.1 Principles

- **Migrate, don't reinvent.** Each Perl `t/` test becomes a Rust test (`#[test]` / `#[tokio::test]`) asserting the same behavior. The assertions encode known-good outputs for thousands of real devices and systems.
- **Reuse fixtures verbatim.** Files under `resources/` (command outputs, SNMP walks, WMI dumps, EDID blobs, plist/XML samples) are copied unchanged into the Rust repo and loaded by tests. Real captures are never replaced with synthetic data.
- **Per-phase, not end-loaded.** Tests for a module are written in the same phase as the module. The CI gate refuses to merge a module without its passing tests.
- **Offline-capable.** Because fixtures are static captures, the whole suite runs on Linux CI with no live devices, no Windows host, and no ESX server.

### 13.2 Mapping Perl tests → Rust

| Perl source | Rust target | Notes |
|---|---|---|
| `t/agent/config*.t` | `glpi-core/tests/config.rs` | precedence, registry, conf.d |
| `t/agent/snmp/*.t`, `mock.t` | `glpi-discovery/tests/snmp_*.rs` | reuse `resources/walks/*` |
| `t/tasks/inventory/generic/**` | `glpi-inventory-local/tests/generic_*.rs` | dmidecode, screen/EDID, printers |
| `t/tasks/inventory/linux/**` | `…/tests/linux_*.rs` | networks, storages, distro |
| `t/tasks/inventory/win32/**` | `…/tests/win32_*.rs` | run against captured `.wmi` dumps |
| `t/tasks/inventory/macos/**` | `…/tests/macos_*.rs` | ifconfig, ioreg, system_profiler |
| `t/tasks/inventory/{hpux,aix,solaris}/**` | `…/tests/{hpux,aix,solaris}_*.rs` | exotic platforms |
| `t/tasks/inventory/virtualization/**` | `…/tests/virt_*.rs` | reuse `resources/virtualization/**` |
| `t/tasks/remoteinventory.t` | `glpi-inventory-remote/tests/` | mock SSH/WinRM transcripts |
| `t/tasks/deploy/**` | `glpi-deploy/tests/` | CheckProcessor cases |
| `t/agent/http/**` | `glpi-http/tests/` | status, /now, trust |
| `t/agent/tools/*.t` | `glpi-core/tests/tools_*.rs` | parsing/normalization helpers |

### 13.3 Fixture handling

- Import the entire `resources/` tree into the workspace under `crates/<crate>/tests/fixtures/` (or a shared `test-fixtures/` crate), preserving relative paths so test data stays traceable to its origin.
- Provide a tiny loader helper (e.g. `fixture("linux/smartctl/sample5")`) so test files read captures uniformly.
- Keep fixtures under version control; treat them as immutable inputs. When upstream adds a new device fixture, mirror it.

### 13.4 Test categories in the Rust suite

1. **Unit / parser tests** — feed a captured command output or SNMP walk to a parser, assert the structured result. The bulk of migrated tests.
2. **Golden-file (parity) tests** — run a full task against a fixture, serialize to JSON, and diff against the normalized output captured from the Perl agent. This is the §0.5 acceptance gate. Normalize volatile fields (timestamps, ordering) before diffing.
3. **Crypto vector tests** — SNMPv3 USM auth/priv validated against RFC 3414 / 7860 published vectors, independent of any device.
4. **Property tests** (`proptest`) — for parsers and the IP-range iterator, to surface edge cases the fixtures don't cover.
5. **Integration tests** — mock GLPI server (wiremock), mock SNMP responder, mock vSphere SOAP endpoint; assert full request/response flows incl. START/DEVICE/STOP and auth.

### 13.5 CI gate

- `cargo test --workspace` runs on every PR across the Linux/Windows/macOS matrix.
- Coverage measured with `cargo-llvm-cov`; merge blocked if coverage for discovery core / NetInventory drops below 80% or if any migrated test is removed without a documented justification.
- A `tests/PARITY.md` map (Perl test file → Rust test, or "intentionally dropped + reason") is kept current and audited in Phase 10.

### 13.6 Test dependencies

| Crate | Use |
|---|---|
| built-in `#[test]` / `#[tokio::test]` | unit + async tests |
| `rstest` | parameterized cases (many fixtures per parser) |
| `insta` | snapshot/golden-file assertions |
| `wiremock` | mock GLPI server / HTTP endpoints |
| `proptest` | property-based testing |
| `cargo-llvm-cov` | coverage measurement in CI |
| `assert_cmd` + `predicates` | CLI black-box tests |
