<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Upstream module mapping

Maps the upstream **Perl** GLPI agent's modules and features to their **Rust**
and **Go** counterparts in this workspace, so upstream fixes and features can be
located and ported deliberately rather than rediscovered each time.

This workspace carries two parallel re-implementations (see the
[Rust migration plan](../glpi-agent-rust-migration-plan.md) and the
[Go implementation plan](../glpi-agent-go-implementation-plan.md)). Both derive
from the **same upstream Perl** source; the Go track in particular is derived
*exclusively* from Perl, never from the Rust code.

This is the **module/feature** layer of upstream tracking. It pairs with:

- [UPSTREAM.md](../UPSTREAM.md) — the pinned upstream **commit** and version mapping,
- [tests/PARITY.md](../tests/PARITY.md) — the **test** (`t/`) parity map.

**Synced to:** upstream `1.17` line, commit `24fec36` (see [UPSTREAM.md](../UPSTREAM.md)).

**Legend:** ✅ ported · 🟡 partial / best-effort · ⬜ missing · 🚫 intentionally dropped.

> **Go track status (Phase map, [plan §4](../glpi-agent-go-implementation-plan.md)).**
> Implemented so far: the single-binary CLI skeleton, `--version`, the inventory
> *document* model, the `inject` (bin/glpi-injector) and `wakeonlan` paths,
> `config` + `logging`, **Phase 8 vSphere/ESX**, the **Phase 7 SSH** remote
> path, the **Phase 10** cross-compile/CI spike, **Phase 2–3** (NetDiscovery +
> NetInventory over SNMP via gosnmp), the **NetInventory MibSupport** framework (first vendor batch), and **Phase 6** (Linux local inventory: ~31 sections, 2 partial
> sections so far). Areas where Go is uniformly
> not started yet carry a per-section Go note below instead of a column of
> identical ⬜ cells; tables where Go already has entries get a full **Go** column.
>
> The §8 bake-off slice (Phase 1 + vSphere + SSH + packaging) is **complete**; its
> measured findings are recorded in
> [ADR-011](adr/ADR-011-go-dual-track-evaluation.md) (the go/no-go decision is
> pending there).

> Status reflects *module/section presence*, not field-by-field depth inside a
> ported section. Where a section is ported but some platform's detail fields
> are best-effort, it is marked 🟡 with a note.

## Tasks

| Upstream `Task/` | Rust crate | Rust | Go package | Go |
| --- | --- | --- | --- | --- |
| `Inventory.pm` | `glpi-inventory-local` | ✅ | `internal/{content,inventory}` | 🟡 document model + 28 Linux sections (bios, hardware, os, cpus, memories, networks, drives, storages, softwares, local_users/groups, envs, batteries, inputs, processes, usbdevices, controllers, videos, sounds, slots, ports, monitors, physical_volumes, volume_groups, logical_volumes, users, printers, firewall) (virtualmachines: all mainstream Linux hypervisors; antivirus: all 8 detectors; remote_mgmt 🟡 TeamViewer+AnyDesk+RustDesk; videos 🟡 lspci); see the local-sections table (dmidecode/lspci/lvm-based categories are done; several parsers are pinned against real upstream captures — see [tests/PARITY.md](../tests/PARITY.md)). DATABASES, modems/powersupplies and the Windows/macOS collectors are pending. Output: local file/stdout, or **sent to a GLPI server** (`--server`, CONTACT + submit — see Server communication) |
| `NetDiscovery.pm` | `glpi-discovery` | ✅ | `internal/discovery` | ✅ SNMP probe (system-MIB device properties + sysObjectID classification) + IPv4 range scan via gosnmp, SNMPv3 (USM auth/priv), a threaded worker pool, and non-SNMP host discovery via the ARP cache (`/proc/net/arp`) + NetBIOS NBSTAT (UDP/137) merged per address (`_scanAddress`) |
| `NetInventory.pm` | `glpi-discovery` | ✅ | `internal/discovery` | 🟡 generic properties + sysObjectID classification (embedded `sysobject.ids`) + SERIAL/FIRMWARE/MAC + IF-MIB PORTS + generic ENTITY-MIB COMPONENTS (incl. Dell/Cisco fix-ups) + NETWORKING port enrichment (TRUNK, AGGREGATE via LACP/PAgP, known-MAC FDB connections) via gosnmp + MibSupport (all 78 device/OS vendor modules + the SnmpFramework engine-id fallback classifier, incl. the run/getComponents device-mutation hooks; only ConfigurationPlugin — a config-time plugin loader — is unported). VLANs and CDP/LLDP/EDP neighbour discovery are the remaining port enrichments |
| `ESX.pm` | `glpi-vsphere` | ✅ | `internal/vsphere` | ✅ via govmomi |
| `RemoteInventory.pm` | `glpi-inventory-remote` | ✅ | `internal/remote` | 🟡 SSH connect/exec + remote document (host/OS/arch); WinRM and full collectors pending |
| `Collect.pm` | `glpi-collect` | ✅ | `internal/collect` | ⬜ Phase 9 |
| `Deploy.pm` | `glpi-deploy` | ✅ | `internal/deploy` | ⬜ Phase 9 |
| `WakeOnLan.pm` | `glpi-wakeonlan` | ✅ | `internal/cli` (wakeonlan) | 🟡 udp method; ethernet (raw L2) deferred |
| _(no upstream equivalent)_ | `glpi-iec61850` | ➕ Rust addition | `internal/iec61850` | ⬜ Phase 4 (build-tagged, cgo) |

## Remote inventory transport

How `RemoteInventory` reaches a host. Upstream supports SSH (system `ssh`,
`Net::SSH2`/libssh2, or a pure-Perl mode) and WinRM.

| Upstream `RemoteInventory/Remote/` | Rust | Go | Go status |
| --- | --- | --- | --- |
| `Ssh.pm` | `glpi-inventory-remote` (russh) | `internal/remote/ssh.go` (`x/crypto/ssh`) | 🟡 connect (password + identity), `LANG=C` exec, OSName/hostname/FQDN/CanRun/ReadFile, host-key policy from `stricthostkeychecking` (strict/accept-new/no) |
| `Winrm.pm` | `glpi-inventory-remote` | `internal/remote` | ⬜ pending (`masterzen/winrm`) |

## Server communication (GLPI protocol)

How the agent talks to a GLPI server: the modern GLPI Agent protocol (CONTACT
handshake + JSON inventory submission). The legacy OCS XML PROLOG/SEND path is
intentionally not ported to Go. ✅ working end to end via `inventory --server`
(covered by an httptest integration test of the CONTACT + submit dialog). The
daemon/scheduling loop (PROLOG_FREQ) and server-driven Collect/Deploy tasks are
out of scope here.

| Upstream | Go | Go status |
| --- | --- | --- |
| `Protocol/{Message,Contact}.pm` | `internal/protocol` | ✅ CONTACT request encode + answer parse (status / expiration / tasks); modern protocol only |
| `HTTP/Client{,/GLPI}.pm` | `internal/transport/glpi.go` | ✅ POST + zlib compress/decompress (zlib/gzip by content-type) + GLPI-Agent-ID; full TLS (no-ssl-check, ca-cert-file/dir, client cert ssl-cert/key), basic auth, proxy (none/explicit/env), timeout. OAuth2/Win-KeyStore out of scope |
| `Target/Server.pm`, `Storage.pm` | `internal/target`, `internal/state` | ✅ server target URL canonicalisation (bare host → http, scheme check) + per-URL subdir; persistent agent id (UUID) **and schedule** (nextRunDate/baseRunDate/backoff) in a JSON state file under the per-server vardir. isGlpiServer is decided per run from the CONTACT answer |
| `Agent.pm` getContact + `Task/Inventory.pm` submit | `internal/cli` (`inventory --server`) | ✅ `glpi-agent --server <url> inventory`: CONTACT handshake → (if inventory enabled) submit the inventory; reads server/TLS/auth/proxy/tag from the global config; clear error when the server is not a modern GLPI server |

## Daemon / scheduling

The periodic agent: a run-loop scheduling each target, honouring the GLPI
server's `expiration` and backing off on network errors. The Go daemon is a
foreground run-loop; the Perl process machinery (fork, PID files, IPC,
daemonize, syslog) is intentionally not ported. ✅ working via the `daemon`
subcommand.

| Upstream | Go | Go status |
| --- | --- | --- |
| `Target.pm` (nextRunDate / maxDelay / delaytime / expiration / backoff) | `internal/scheduler` | ✅ run timing: jittered computeNextRunDate, ResetNextRunDate, SetNextRunOnExpiration, exponential BackOff, Trigger (run-now); clock/rng injectable. Snapshot/Restore persists nextRunDate/baseRunDate/backoff across restarts (kept when planned within the last maxDelay) |
| target execution (`Task/Inventory` + `getContact`) | `internal/agent` | ✅ `BuildInventory` (collect + tag) and `RunServerTarget` (CONTACT + submit, returns the server expiration); shared by `inventory --server` and the daemon |
| `Daemon.pm` run-loop + `GLPI::Agent` (getTargets/getStatus/run-now) | `internal/agent` (`Agent`) + `internal/cli` (`daemon`) | ✅ `glpi-agent --server <url>[,...] daemon`: the `Agent` owns the targets, run state (Status) and run-now trigger (thread-safe for the control server); one scheduled target per server, run when due, reschedule by expiration / backoff / interval; sleeps until the earliest next run; SIGINT/SIGTERM stop, SIGUSR1 run-now (unix). No fork/PID/IPC/daemonize |

## Local inventory sections

Upstream organises inventory modules by OS (`Task/Inventory/{Generic,Linux,Win32,
MacOS,…}`); here they are mapped by the **GLPI JSON section** the data lands in,
which is how the Rust side ([`glpi-inventory-local`](../crates/glpi-inventory-local/src/categories/),
emitted via [`content.rs`](../crates/glpi-inventory-local/src/content.rs)) is organised.

> **Go:** local inventory collection is **Phase 6, in progress** (Linux first).
> The **Go** column below tracks `internal/inventory`. Done sections read pure
> files/sysfs (no external tools); the ⬜ ones still pending are mostly those that
> need an external command (dmidecode/lspci/lvm) or EDID parsing, noted inline.
> Windows/macOS collectors are separate (a non-Linux stub yields only the
> hostname).

| GLPI section | Upstream module(s) | Rust | Go (Linux) |
| --- | --- | --- | --- |
| `bios` | `Generic/Dmidecode`, `*/Bios` | ✅ | ✅ sysfs DMI |
| `hardware` | `*/Hardware`, `*/Memory` | ✅ | ✅ name + MEMORY/SWAP |
| `operatingsystem` | `Generic/OS`, `*/OS` | ✅ | ✅ os-release + kernel |
| `cpus` | `*/CPU` | ✅ | ✅ /proc/cpuinfo |
| `memories` | `*/Memory` | ✅ | ✅ dmidecode type 17 |
| `softwares` | `Generic/Softwares/*` | ✅ | ✅ dpkg + rpm |
| `networks` | `Generic/Networks`, `*/Networks` | ✅ | ✅ net.Interfaces + sysfs |
| `storages` | `Generic/Storages/*` | ✅ | ✅ /sys/block |
| `drives` | `Generic/Drives` (filesystems) | ⬜ | ✅ /proc/mounts + statfs |
| `local_users` / `local_groups` | `Generic/Users` (local accounts) | ⬜ | ✅ /etc/passwd + /etc/group |
| `envs` | environment variables | ✅ | ✅ |
| `batteries` | `Generic/Batteries/*` | ✅ | ✅ sysfs power_supply |
| `inputs` | `{Win32,Linux}/Inputs` | ⬜ | ✅ /proc/bus/input/devices |
| `processes` | `Generic/Processes` | ✅ | ✅ /proc (PID/USER/CMD/MEM/STARTED; CPUUSAGE/TTY pending) |
| `usbdevices` | `Generic/USB` | ✅ | ✅ /sys/bus/usb |
| `users` | `Generic/Users` (logged-in) | ✅ | ✅ who --users |
| `controllers` | `Win32/Controllers`, PCI | 🟡 | ✅ lspci -v -nn |
| `videos` | `*/Videos` | ✅ | 🟡 lspci (X11 resolution pending) |
| `sounds` | `*/Sounds` | ✅ | ✅ lspci |
| `monitors` | `Generic/Screen` (EDID) | ✅ | ✅ EDID (drm sysfs) + embedded edid.ids |
| `printers` | `Generic/Printers/*` | ✅ | ✅ /etc/cups/printers.conf |
| `slots` / `ports` | `Generic/Dmidecode`, `Win32/*` | 🟡 | ✅ dmidecode types 9/8 |
| `modems` / `powersupplies` | `Win32/Modems`, `MacOS/Psu`, dmidecode | ⬜ | ⬜ dmidecode parser ready |
| `antivirus` | `{Linux,Win32,MacOS}/AntiVirus/*` | 🟡 | ✅ all 8 Linux detectors (Defender, CrowdStrike, Bitdefender, Cortex, Dr.Web, ESET/EEA, KESL, SentinelOne) |
| `physical_volumes` / `volume_groups` / `logical_volumes` | `Linux/LVM` | ⬜ | ✅ pvs/vgs/lvs |
| `virtualmachines` | `Virtualization/*`, `Vmsystem` | ⬜ | ✅ all mainstream Linux hypervisors: libvirt, docker, virtualbox, systemd-nspawn, xen, virtuozzo, qemu, lxd, lxc, vserver. (vmware-workstation/xen-citrix niche; wsl/parallels/hpvm/hyperv/jails/solariszones non-Linux; ESX VMs are in `internal/vsphere`) |
| `licenseinfos` | `Win32/License`, `MacOS/License` | ⬜ | — not a Linux category (Win32/macOS only upstream) |
| `remote_mgmt` | `Generic/Remote_Mgmt/*` | ⬜ | 🟡 TeamViewer + AnyDesk + RustDesk; other agents pending (upstream has no IPMI module) |
| `databases_services` | `Generic/Databases/*` | ⬜ | ⬜ needs live DB connections + credentials (MySQL/PostgreSQL/Oracle/…) |
| `firewall` | `Generic/Firewall`, `*/Firewall` | ⬜ | ✅ ufw + firewalld |
| `accesslog` | `Inventory/AccessLog` | 🚫 | 🚫 minor metadata |
| `rudder` | `Generic/Rudder` | 🚫 | 🚫 Rudder integration |

## Platform inventory coverage

The **Rust** and **Go** columns track each track's per-OS collector coverage
(`cfg(target_os)` in Rust, `//go:build` tags in Go). Linux Go detail is in the
local inventory sections table above.

| OS | Upstream | Rust | Go |
| --- | --- | --- | --- |
| Linux | `Inventory/Linux/**` | ✅ (`linux`) | 🟡 28 sections + 3 partial (`//go:build linux`) |
| Windows | `Inventory/Win32/**` (28) | 🟡 implemented; some detail fields best-effort (`windows`) | ✅ **all 26 standard Win32 inventory categories** mapped via WMI/CIM (PowerShell `Get-CimInstance`/`Invoke-CimMethod` → `ConvertTo-Json`, `collect_windows.go`; CIM `[datetime]` props are normalised to the canonical `YYYY-MM-DD HH:MM:SS` at the PowerShell source so the JSON is deterministic across PS editions, and `wmiDateTime` also defensively parses the ISO-8601-`T` / `/Date(ms)/` forms), pure mappers unit-tested on Linux against CIM-JSON/XML fixtures. The only un-ported Win32 modules are `Registry.pm` (server-directed arbitrary registry-value collection — a config feature, not a category) and `HP.pm` (vendor-specific enrichment). Done: operatingsystem, hardware, bios, cpus, memories, drives, storages, controllers, networks, videos, sounds, slots, ports, softwares (registry), printers, processes, antivirus (SecurityCenter2 productState decode; vendor version/expiration enrichment is follow-on), environment, local_users/local_groups + last-logged user (Win32_UserAccount/Group/ComputerSystem; logged-users-via-Explorer GetOwner and AzureAD/registry fallback are follow-on), inputs (Win32_Keyboard+PointingDevice), modems (Win32_POTSModem), chassis type (Win32_SystemEnclosure), batteries (powercfg /batteryreport XML — parser pinned against the two real upstream captures), monitors (Win32_DesktopMonitor + root/wmi WMIMonitorConnectionParams + registry EDID via the shared EDID parser; PORT/ALTSERIAL follow-on), firewall (per-profile EnableFirewall registry DWORD -> STATUS/PROFILE; per-connection CONNECTIONS/IPADDRESS association follow-on), usbdevices (CIM_LogicalDevice + the vendored usb.ids DB embedded into the Windows build; vendor/device names pinned against the upstream usb.t cases; dock/vendor sub-module enrichment follow-on), process/user **GetOwner** (Win32_Process owner via Invoke-CimMethod: PROCESSES USER with local/NT-AUTHORITY domain rules, plus the interactive logged-in USERS from Explorer.exe owners merged with the last user), licenseinfos (Office-registry product keys via the `decodeMicrosoftKey` DigitalProductID decoder — **pinned against the real upstream `license.t` binary vectors** — merged with the SoftwareLicensingProduct WMI source per the upstream seenProducts/PRODUCTCODE merge; OS-license skip, lc(ID) dedupe + NAME/FULLNAME/KEY sort) plus the **Adobe** cache.db source (`getAdobeLicensesWithoutSqlite` — the `_decodeAdobeKey` cipher + the FLMap/SN regex parse, pinned against the real upstream capture and verified byte-faithful to the upstream regex path, which diverges from the SQLite-path values in `license.t`). Remaining: process CPU/MEM perf counters, firewall-connection association, AzureAD user refinement. |
| macOS | `Inventory/MacOS/**` (25) | 🟡 implemented; some detail fields best-effort (`macos`) | 🟡 darwin collector (`collect_darwin.go`) via `system_profiler`/`sysctl`/`ioreg`/`uname`; the `system_profiler` text parser (`getSystemProfilerInfos` port) + pure mappers are unit-tested on Linux against the real `resources/macos/**` captures. Done: operatingsystem (FULL_NAME/VERSION + uname kernel/arch), hardware (NAME + Hardware UUID, ioreg fallback). Remaining: cpus, memories, bios, storages, drives, networks, usb, videos, sound, batteries/psu, softwares, firewall, antivirus. |
| Solaris | `Inventory/Solaris/**` (11) | ⬜ fehlt | ⬜ fehlt |
| AIX | `Inventory/AIX/**` (15) | ⬜ fehlt | ⬜ fehlt |
| HP-UX | `Inventory/HPUX/**` (13) | ⬜ fehlt | ⬜ fehlt |
| *BSD | `Inventory/BSD/**` (13) | ⬜ fehlt | ⬜ fehlt |

## SNMP — standard MIBs

> **Go:** 🟡 (`gosnmp`). The **system MIB** (generic device properties), the
> **IF-MIB** interface table (PORTS, via SNMP walk), the **device
> classification** (sysObjectID matched against the embedded `sysobject.ids`),
> the Entity/Printer SERIAL/FIRMWARE/MODEL OIDs, the ENTITY-MIB COMPONENTS and
> the full per-vendor MibSupport overrides are read by `internal/discovery`,
> along with the NETWORKING port enrichment (TRUNK, AGGREGATE, known-MAC FDB
> connections). VLANs and the LLDP/CDP/EDP neighbour discovery are ⬜ pending.

| Area | Rust | Go | Status |
| --- | --- | --- | --- |
| system | `mib/system_mib.rs` | `discovery` generic properties | ✅ Go |
| interfaces (IF-MIB) | `mib/if_mib.rs` | `discovery` PORTS (walk) | 🟡 Go core columns |
| ip | `mib/ip_mib.rs` | — | ⬜ Go |
| bridge / LLDP / CDP | `mib/{bridge_mib,lldp_mib,cdp_mib}.rs` | `discovery/netports.go` (TRUNK / AGGREGATE / known-MAC FDB) | 🟡 Go port enrichment; VLANs + LLDP/CDP/EDP pending |
| entity / printer | `mib/{entity_mib,printer_mib}.rs` | `discovery` SERIAL/FIRMWARE/MODEL (entPhysical*, prt*) + `components.go` COMPONENTS | ✅ Go device fields + ENTITY-MIB physical components |
| device classification | `mib/device.rs` | `discovery/classify` + `mibsupport` | ✅ Go (sysObjectID DB); MibSupport overrides 🟡 (framework, incl. run/getComponents hooks, + all 78 device/OS modules + SnmpFramework fallback) |

## SNMP — vendor `MibSupport`

Upstream ships 80 `MibSupport/` modules (78 device/OS + 2 framework). The **Rust**
and **Go** columns below track each track's port; the per-module Go detail now
lives in the table rather than in prose. (The **Rust** column ports 69 device
MIBs, 9 missing.) This table is generated — see "Keeping this current".

> **Go:** ✅ **complete** for inventory purposes. `internal/discovery/mibsupport.go`
> ports the MibSupport dispatcher (sysObjectID + sysORID + privateoid matching,
> priority, per-field override) plus the device-mutation hooks `Components`
> (getComponents → COMPONENTS, with a FIRMWARES rewrite) and `Run` (runMibSupport),
> wired into `GetInventory` in the upstream order (setComponents → runMibSupport).
> All **78 device/OS modules** are ported verbatim from the upstream
> `MibSupport/**` OIDs (`mib_vendors*.go`, `linuxappliance.go`), as is the
> SnmpFramework engine-id fallback classifier. Vendor `Run` hooks include Xerox
> (PAGECOUNTERS), Netgear (stacked-chassis serials), Cisco port-security MACs and
> Ubnt (WiFi radio ports → IFTYPE 71, IFALIAS, IFNAME = SSID + band/VLAN). The generic ENTITY-MIB
> physical-components walk (`SNMP/Device/Components.pm` → `components.go`) backs the
> getComponents hooks. The only unported file is `ConfigurationPlugin.pm` — a
> config-time loader for user-supplied MIB modules, not a device MIB, so it has no
> inventory effect.

| Upstream `MibSupport/` | Rust `…/mib/vendor/` | Rust | Go |
| --- | --- | --- | --- |
| `Aerohive.pm` | `aerohive.rs` | ✅ | ✅ |
| `Akcp.pm` | `akcp.rs` | ✅ | ✅ |
| `Aruba.pm` | — | ⬜ fehlt | ✅ |
| `Avaya.pm` | `avaya.rs` | ✅ | ✅ |
| `Avocent.pm` | `avocent.rs` | ✅ | ✅ |
| `Bachmann.pm` | `bachmann.rs` | ✅ | ✅ |
| `Brocade.pm` | `brocade.rs` | ✅ | ✅ |
| `BrotherNetConfig.pm` | `brother.rs` | ✅ | ✅ |
| `Canon.pm` | `canon.rs` | ✅ | ✅ |
| `CheckPoint.pm` | `checkpoint.rs` | ✅ | ✅ |
| `Cisco.pm` | `cisco.rs` | ✅ | ✅ |
| `CiscoMeraki.pm` | `cisco_meraki.rs` | ✅ | ✅ |
| `CiscoPortSecurity.pm` | — | ⬜ fehlt | ✅ |
| `CiscoUcsBoard.pm` | `cisco_ucs_board.rs` | ✅ | ✅ |
| `CitrixNetscaler.pm` | `netscaler.rs` | ✅ | ✅ |
| `ConfigurationPlugin.pm` | core SNMP framework | 🟡 infra (not a 1:1 file) | ⬜ plugin loader |
| `DefencePro.pm` | `defencepro.rs` | ✅ | ✅ |
| `Dell.pm` | `dell.rs` | ✅ | ✅ |
| `Digi.pm` | — | ⬜ fehlt | ✅ |
| `DigiPower.pm` | `digipower.rs` | ✅ | ✅ |
| `Dlink.pm` | `dlink.rs` | ✅ | ✅ |
| `DlinkDGS1210Series.pm` | `dlink_dgs1210.rs` | ✅ | ✅ |
| `EatonEpdu.pm` | `eaton.rs` | ✅ | ✅ |
| `EMC.pm` | `emc.rs` | ✅ | ✅ |
| `Epson.pm` | `epson.rs` | ✅ | ✅ |
| `Force10S.pm` | — | ⬜ fehlt | ✅ |
| `Fortinet.pm` | `fortinet.rs` | ✅ | ✅ |
| `FoxGate.pm` | `foxgate.rs` | ✅ | ✅ |
| `FreeBSD.pm` | — | ⬜ fehlt | ✅ |
| `Hikvision.pm` | `hikvision.rs` | ✅ | ✅ |
| `HitachiVantara.pm` | `hitachi_vantara.rs` | ✅ | ✅ |
| `HPCitizen.pm` | `hp_citizen.rs` | ✅ | ✅ |
| `HPHttpManagement.pm` | `hp_http_management.rs` | ✅ | ✅ |
| `HPNetPeripheral.pm` | `hp_printer.rs` | ✅ | ✅ |
| `Htek.pm` | `htek.rs` | ✅ | ✅ |
| `Hwg.pm` | `hwg.rs` | ✅ | ✅ |
| `Idrac.pm` | `idrac.rs` | ✅ | ✅ |
| `IEEE802dot11.pm` | — | ⬜ fehlt | ✅ |
| `iLO.pm` | `ilo.rs` | ✅ | ✅ |
| `Infortrend.pm` | `infortrend.rs` | ✅ | ✅ |
| `Intelbras.pm` | `intelbras.rs` | ✅ | ✅ |
| `Juniper.pm` | `juniper.rs` | ✅ | ✅ |
| `Konica.pm` | `konica.rs` | ✅ | ✅ |
| `Kyocera.pm` | `kyocera.rs` | ✅ | ✅ |
| `Lexmark.pm` | `lexmark.rs` | ✅ | ✅ |
| `LinuxAppliance.pm` | — | ⬜ fehlt | ✅ |
| `Meinberg.pm` | `meinberg.rs` | ✅ | ✅ |
| `Mikrotik.pm` | `mikrotik.rs` | ✅ | ✅ |
| `Multitech.pm` | `multitech.rs` | ✅ | ✅ |
| `Netgear.pm` | — | ⬜ fehlt | ✅ |
| `Nokia.pm` | `nokia.rs` | ✅ | ✅ |
| `Oki.pm` | `oki.rs` | ✅ | ✅ |
| `Panasas.pm` | — | ⬜ fehlt | ✅ |
| `Pantum.pm` | `pantum.rs` | ✅ | ✅ |
| `Qnap.pm` | `qnap.rs` | ✅ | ✅ |
| `Quantum.pm` | `quantum.rs` | ✅ | ✅ |
| `Radware.pm` | `radware.rs` | ✅ | ✅ |
| `Raritan.pm` | `raritan.rs` | ✅ | ✅ |
| `Ricoh.pm` | `ricoh.rs` | ✅ | ✅ |
| `RNX.pm` | `rnx.rs` | ✅ | ✅ |
| `Ruckus.pm` | `ruckus.rs` | ✅ | ✅ |
| `Siemens.pm` | `siemens.rs` | ✅ | ✅ |
| `SiemensSicam.pm` | `siemens_sicam.rs` | ✅ | ✅ |
| `SnmpFramework.pm` | core SNMP framework | 🟡 infra (not a 1:1 file) | ✅ |
| `Snom.pm` | `snom.rs` | ✅ | ✅ |
| `SonicWall.pm` | `sonicwall.rs` | ✅ | ✅ |
| `Sophos.pm` | `sophos.rs` | ✅ | ✅ |
| `Telco.pm` | `telco.rs` | ✅ | ✅ |
| `Tiesse.pm` | `tiesse.rs` | ✅ | ✅ |
| `Toshiba.pm` | `toshiba.rs` | ✅ | ✅ |
| `TpLink.pm` | `tplink.rs` | ✅ | ✅ |
| `Ubnt.pm` | `ubnt.rs` | ✅ | ✅ |
| `UPS.pm` | `ups.rs` | ✅ | ✅ |
| `Voltaire.pm` | `voltaire.rs` | ✅ | ✅ |
| `Voltronic.pm` | `voltronic.rs` | ✅ | ✅ |
| `WatchGuard.pm` | `watchguard.rs` | ✅ | ✅ |
| `WyseThinOS.pm` | `wyse_thinos.rs` | ✅ | ✅ |
| `Xerox.pm` | `xerox.rs` | ✅ | ✅ |
| `Zebra.pm` | `zebra.rs` | ✅ | ✅ |
| `Zyxel.pm` | `zyxel.rs` | ✅ | ✅ |

## HTTP control server

The core control endpoints are ported in Go (`internal/httpd`, served by the
daemon): the `/status`, `/now` (run-now, gated by the httpd-trust IP allowlist)
and root status page, querying the `agent.Agent`. The proxy/deploy plugins, the
CORS/event machinery on `/now`, and the ToolBox web GUI are not ported.

| Upstream `HTTP/Server{,.pm}` | Rust `glpi-http` | Rust | Go |
| --- | --- | --- | --- |
| `Server.pm` `/status`, `/now`, `/` + trust | `server.rs` | ✅ | ✅ `internal/httpd`: /status (text), /now (run-now if trusted), root status page; httpd-trust IP/CIDR allowlist. Served by the `daemon` on httpd-ip:httpd-port (default 62354) unless `--no-httpd`; graceful shutdown with the daemon |
| `Proxy.pm` | `proxy.rs` | 🟡 ported; `GLPI-Proxy-ID` hop header not forwarded ([proxy.rs](../crates/glpi-http/src/proxy.rs)) | ⬜ not ported |
| `SSL.pm` | `tls.rs` | ✅ | ✅ HTTPS listener: a server cert via `httpd-ssl-cert-file`/`httpd-ssl-key-file` upgrades the control server to TLS (else plain HTTP) |
| `BasicAuthentication.pm` | — (only IP trust in `trust.rs`) | ⬜ fehlt | ⬜ IP trust only |
| `SecondaryProxy.pm` | — | ⬜ fehlt | ⬜ not ported |
| `Test.pm` | — | ⬜ minor | ⬜ minor |
| `ToolBox.pm` | — | 🚫 web GUI, intentionally dropped | 🚫 web GUI, intentionally dropped |

## Configuration sources

| Upstream layer | Rust (`glpi-core::config`) | Rust | Go (`internal/config`) | Go |
| --- | --- | --- | --- | --- |
| defaults → `agent.cfg` → `conf.d/*.cfg` (`include`) → CLI | `config/{sources,mod,options}.rs` | ✅ | `internal/config` | ✅ defaults < file (`include`) < CLI + `_checkContent` |
| env layer | `config/sources.rs` | ✅ | — | ⬜ |
| Windows registry | — | ⬜ fehlt ([config/mod.rs](../crates/glpi-core/src/config/mod.rs)) | — | ⬜ Phase 6 (`x/sys/windows/registry`) |
| logging backends (Stderr/File/Syslog) | `glpi-core::logging` | ✅ | `internal/logging` | 🟡 Stderr + File; Syslog deferred |

## Build & packaging

Both tracks target the same release matrix — Linux/Windows/macOS on x86_64 and
aarch64 (see [release.yml](../.github/workflows/release.yml)).

| Concern | Rust | Go | Go status |
| --- | --- | --- | --- |
| Cross-compile matrix | per-triple `rustup target` + toolchain | `go/scripts/cross-build.sh` (`GOOS/GOARCH`) | ✅ all six targets, pure-Go static (`CGO_ENABLED=0`) from one host |
| CI (lint/test) | `ci.yml` (fmt + clippy, per-OS test) | [`go.yml`](../.github/workflows/go.yml) | ✅ gofmt+vet, per-OS test, cross matrix |
| deb / rpm / msi / pkg | `release.yml` (cargo-deb, generate-rpm, WiX, pkgbuild) | — | ⬜ pending (binaries built; OS packages next) |
| snap / flatpak | `release.yml` + [`packaging/`](../packaging/) | — | ⬜ pending |
| IEC 61850 (cgo) impact on cross-build | feature-gated FFI | build-tagged, off by default | ✅ keeps the default build cgo-free |

## Keeping this current

This file is part of the pin-bump checklist in [UPSTREAM.md](../UPSTREAM.md): when
the synced commit moves, reconcile this mapping in the same PR — for **both** the
Rust and Go columns/notes, so each track's parity against upstream stays visible.

As the Go track advances through its phases, promote the per-section Go notes to
full **Go** columns once a section has at least one non-⬜ entry (the Tasks and
Configuration tables already use full columns).

The vendor-MIB table is mechanical and should be regenerated rather than
hand-edited. Against a checkout of the pinned upstream commit and this repo,
list both sides and diff:

```sh
# upstream MibSupport module names (strip the .pm)
ls path/to/glpi-agent/lib/GLPI/Agent/SNMP/MibSupport/*.pm | xargs -n1 basename | sed 's/\.pm$//'
# this repo's ported vendor MIBs
ls crates/glpi-discovery/src/snmp/mib/vendor/*.rs | xargs -n1 basename | sed 's/\.rs$//'
```

A new upstream module with no row here is a freshly-introduced gap; add it with
status ⬜ (or 🚫 with a reason if deliberately skipped) so it is not silently
lost.
