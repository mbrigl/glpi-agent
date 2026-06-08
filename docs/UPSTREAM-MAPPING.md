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
| `Inventory.pm` | `glpi-inventory-local` | ✅ | `internal/{content,inventory}` | 🟡 document model + 28 Linux sections (bios, hardware, os, cpus, memories, networks, drives, storages, softwares, local_users/groups, envs, batteries, inputs, processes, usbdevices, controllers, videos, sounds, slots, ports, monitors, physical_volumes, volume_groups, logical_volumes, users, printers, firewall) (virtualmachines: all mainstream Linux hypervisors; antivirus: all 8 detectors; remote_mgmt 🟡 TeamViewer+AnyDesk+RustDesk; videos 🟡 lspci); see the local-sections table. dmidecode/lspci/lvm-based categories and Windows/macOS pending |
| `NetDiscovery.pm` | `glpi-discovery` | ✅ | `internal/discovery` | ✅ SNMP probe (system-MIB device properties + sysObjectID classification) + IPv4 range scan via gosnmp, SNMPv3 (USM auth/priv), a threaded worker pool, and non-SNMP host discovery via the ARP cache (`/proc/net/arp`) + NetBIOS NBSTAT (UDP/137) merged per address (`_scanAddress`) |
| `NetInventory.pm` | `glpi-discovery` | ✅ | `internal/discovery` | 🟡 generic properties + sysObjectID classification (embedded `sysobject.ids`) + SERIAL/FIRMWARE/MAC + IF-MIB PORTS + generic ENTITY-MIB COMPONENTS (incl. Dell/Cisco fix-ups) via gosnmp + MibSupport (all 78 device/OS vendor modules + the SnmpFramework engine-id fallback classifier, incl. the run/getComponents device-mutation hooks; only ConfigurationPlugin — a config-time plugin loader — is unported) |
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

> **Go:** Linux 🟡 (28 sections + 3 partial via `//go:build linux` collectors — see the local
> inventory sections table); Windows/macOS ⬜ (a non-Linux stub collects only the
> hostname).

| OS | Upstream | Rust (`cfg(target_os)`) | Status |
| --- | --- | --- | --- |
| Linux | `Inventory/Linux/**` | `linux` | ✅ |
| Windows | `Inventory/Win32/**` | `windows` | 🟡 implemented; some detail fields best-effort |
| macOS | `Inventory/MacOS/**` | `macos` | 🟡 implemented; some detail fields best-effort |
| Solaris | `Inventory/Solaris/**` (11) | — | ⬜ fehlt |
| AIX | `Inventory/AIX/**` (15) | — | ⬜ fehlt |
| HP-UX | `Inventory/HPUX/**` (13) | — | ⬜ fehlt |
| *BSD | `Inventory/BSD/**` (13) | — | ⬜ fehlt |

## SNMP — standard MIBs

> **Go:** 🟡 (`gosnmp`). The **system MIB** (generic device properties), the
> **IF-MIB** interface table (PORTS, via SNMP walk), the **device
> classification** (sysObjectID matched against the embedded `sysobject.ids`),
> and the Entity/Printer SERIAL/FIRMWARE/MODEL OIDs are read by
> `internal/discovery`. ip/bridge/LLDP/CDP and the per-vendor MibSupport
> overrides are ⬜ pending.

| Area | Rust | Go | Status |
| --- | --- | --- | --- |
| system | `mib/system_mib.rs` | `discovery` generic properties | ✅ Go |
| interfaces (IF-MIB) | `mib/if_mib.rs` | `discovery` PORTS (walk) | 🟡 Go core columns |
| ip | `mib/ip_mib.rs` | — | ⬜ Go |
| bridge / LLDP / CDP | `mib/{bridge_mib,lldp_mib,cdp_mib}.rs` | — | ⬜ Go |
| entity / printer | `mib/{entity_mib,printer_mib}.rs` | `discovery` SERIAL/FIRMWARE/MODEL (entPhysical*, prt*) + `components.go` COMPONENTS | ✅ Go device fields + ENTITY-MIB physical components |
| device classification | `mib/device.rs` | `discovery/classify` + `mibsupport` | ✅ Go (sysObjectID DB); MibSupport overrides 🟡 (framework, incl. run/getComponents hooks, + all 78 device/OS modules + SnmpFramework fallback) |

## SNMP — vendor `MibSupport`

Upstream ships 80 `MibSupport/` modules (78 device/OS + 2 framework); 69 device
MIBs are ported, **9 are missing**, and the 2 framework modules are infra that
the Rust SNMP core handles differently. This table is generated — see
"Keeping this current".

> **Go:** 🟡 **in progress**. `internal/discovery/mibsupport.go` ports the
> MibSupport dispatcher (sysObjectID + sysORID + privateoid matching, priority, per-field
> override). Ported so far (mib_vendors*.go): Mikrotik, Ubnt, Dell, Fortinet, Cisco, Juniper,
> HP, Brother, Canon, Epson, Konica, Ricoh, Kyocera, Lexmark, Zebra, Aruba, Avaya, Brocade, CheckPoint, Dlink, Hikvision, iLO, iDRAC, Nokia, SonicWall, Sophos, TpLink, Zyxel, OKI, Qnap, Ruckus, CiscoMeraki, Eaton, Raritan, Snom, Htek, WatchGuard, WyseThinOS, Intelbras, Pantum, Toshiba, Hwg, Meinberg, Avocent, Bachmann, CiscoUCS, DefensePro, DigiPower, FoxGate, Hitachi, HP-HTTP, Infortrend, Multitech, Quantum, Radware, UPS(APC/Riello/std), Voltronic, Aerohive, Akcp, NetScaler, Dlink-DGS1210, Digi, HP-Citizen, RNX, Telco, Tiesse, Voltaire, SiemensSicam, Xerox, Netgear, EMC, Force10S, Panasas, Siemens, FreeBSD/Stormshield, LinuxAppliance, CiscoPortSecurity, IEEE802dot11 **(all 78 device/OS modules)**,
> each verbatim from the upstream `MibSupport/**` OIDs (not the Rust files).
> Matching covers sysObjectID, sysORID and privateoid rules. The framework now also
> ports the device-mutation hooks: `Components` (getComponents → COMPONENTS, with a
> FIRMWARES rewrite) and `Run` (runMibSupport), wired into `GetInventory` after the
> identity fields and ports in the upstream order (setComponents → runMibSupport).
> This covers Xerox (PAGECOUNTERS), SiemensSicam (DGPI components + firmwares) and
> Netgear (stacked-chassis serial/STACK_NUMBER fix-up). The generic ENTITY-MIB
> physical-components walk (`SNMP/Device/Components.pm` — `components.go`,
> `BuildPhysicalComponents`) is also ported and runs before the MibSupport
> getComponents accessors, so Netgear's run hook now fixes up real chassis
> components. The index-/conditional-logic batch is also ported: EMC (FCMGMT
> connUnit table), Force10S (stack/port getComponents), Panasas (member serial
> keyed by the device IP), Siemens (iASi-Link + sysDescr fallback) and
> FreeBSD/Stormshield. LinuxAppliance is also ported (`linuxappliance.go`): the
> ordered appliance detection (Seagate/QuesCom/Synology/CheckPoint/Sophos/UniFi/
> Socomec/Quantum/Digi/TP-Link/printer) plus the snmpEngineID IANA-manufacturer
> decode — reusing the embedded sysobject.ids DB as `getManufacturerIDInfo` — and
> the run enrichment (Synology disks→STORAGES / volumes→DRIVES, per-vendor
> firmware). Detection state is stashed under a private `_appliance` device key
> that GetInventory strips before output. The final two enrichment modules are
> also done: CiscoPortSecurity (a run hook attaching each port's secure MAC as a
> PORT connection) and IEEE802dot11 (a priority-50 module filling
> MANUFACTURER/MODEL/FIRMWARE from the dot11 resource table only when the generic
> classification left them empty, incl. the Ubnt version extract). That completes
> all 78 device/OS MibSupport modules. SnmpFramework is also ported
> (`mib_vendors9.go`): a priority-100 last-resort classifier that fills
> MANUFACTURER/MODEL/SERIAL from the IANA manufacturer id decoded out of the
> snmpEngineID (sharing `snmpEngineIDInfo` with LinuxAppliance), only when nothing
> more specific provided them. Only ConfigurationPlugin remains unported — it is a
> config-time plugin loader, not a device MIB, so it has no inventory effect.

| Upstream `MibSupport/` | Rust `…/mib/vendor/` | Status |
| --- | --- | --- |
| `Aerohive.pm` | `aerohive.rs` | ✅ |
| `Akcp.pm` | `akcp.rs` | ✅ |
| `Aruba.pm` | — | ⬜ fehlt |
| `Avaya.pm` | `avaya.rs` | ✅ |
| `Avocent.pm` | `avocent.rs` | ✅ |
| `Bachmann.pm` | `bachmann.rs` | ✅ |
| `Brocade.pm` | `brocade.rs` | ✅ |
| `BrotherNetConfig.pm` | `brother.rs` | ✅ |
| `Canon.pm` | `canon.rs` | ✅ |
| `CheckPoint.pm` | `checkpoint.rs` | ✅ |
| `Cisco.pm` | `cisco.rs` | ✅ |
| `CiscoMeraki.pm` | `cisco_meraki.rs` | ✅ |
| `CiscoPortSecurity.pm` | — | ⬜ fehlt |
| `CiscoUcsBoard.pm` | `cisco_ucs_board.rs` | ✅ |
| `CitrixNetscaler.pm` | `netscaler.rs` | ✅ |
| `ConfigurationPlugin.pm` | core SNMP framework | 🟡 infra (not a 1:1 file) |
| `DefencePro.pm` | `defencepro.rs` | ✅ |
| `Dell.pm` | `dell.rs` | ✅ |
| `Digi.pm` | — | ⬜ fehlt |
| `DigiPower.pm` | `digipower.rs` | ✅ |
| `Dlink.pm` | `dlink.rs` | ✅ |
| `DlinkDGS1210Series.pm` | `dlink_dgs1210.rs` | ✅ |
| `EatonEpdu.pm` | `eaton.rs` | ✅ |
| `EMC.pm` | `emc.rs` | ✅ |
| `Epson.pm` | `epson.rs` | ✅ |
| `Force10S.pm` | — | ⬜ fehlt |
| `Fortinet.pm` | `fortinet.rs` | ✅ |
| `FoxGate.pm` | `foxgate.rs` | ✅ |
| `FreeBSD.pm` | — | ⬜ fehlt |
| `Hikvision.pm` | `hikvision.rs` | ✅ |
| `HitachiVantara.pm` | `hitachi_vantara.rs` | ✅ |
| `HPCitizen.pm` | `hp_citizen.rs` | ✅ |
| `HPHttpManagement.pm` | `hp_http_management.rs` | ✅ |
| `HPNetPeripheral.pm` | `hp_printer.rs` | ✅ |
| `Htek.pm` | `htek.rs` | ✅ |
| `Hwg.pm` | `hwg.rs` | ✅ |
| `Idrac.pm` | `idrac.rs` | ✅ |
| `IEEE802dot11.pm` | — | ⬜ fehlt |
| `iLO.pm` | `ilo.rs` | ✅ |
| `Infortrend.pm` | `infortrend.rs` | ✅ |
| `Intelbras.pm` | `intelbras.rs` | ✅ |
| `Juniper.pm` | `juniper.rs` | ✅ |
| `Konica.pm` | `konica.rs` | ✅ |
| `Kyocera.pm` | `kyocera.rs` | ✅ |
| `Lexmark.pm` | `lexmark.rs` | ✅ |
| `LinuxAppliance.pm` | — | ⬜ fehlt |
| `Meinberg.pm` | `meinberg.rs` | ✅ |
| `Mikrotik.pm` | `mikrotik.rs` | ✅ |
| `Multitech.pm` | `multitech.rs` | ✅ |
| `Netgear.pm` | — | ⬜ fehlt |
| `Nokia.pm` | `nokia.rs` | ✅ |
| `Oki.pm` | `oki.rs` | ✅ |
| `Panasas.pm` | — | ⬜ fehlt |
| `Pantum.pm` | `pantum.rs` | ✅ |
| `Qnap.pm` | `qnap.rs` | ✅ |
| `Quantum.pm` | `quantum.rs` | ✅ |
| `Radware.pm` | `radware.rs` | ✅ |
| `Raritan.pm` | `raritan.rs` | ✅ |
| `Ricoh.pm` | `ricoh.rs` | ✅ |
| `RNX.pm` | `rnx.rs` | ✅ |
| `Ruckus.pm` | `ruckus.rs` | ✅ |
| `Siemens.pm` | `siemens.rs` | ✅ |
| `SiemensSicam.pm` | `siemens_sicam.rs` | ✅ |
| `SnmpFramework.pm` | core SNMP framework | 🟡 infra (not a 1:1 file) |
| `Snom.pm` | `snom.rs` | ✅ |
| `SonicWall.pm` | `sonicwall.rs` | ✅ |
| `Sophos.pm` | `sophos.rs` | ✅ |
| `Telco.pm` | `telco.rs` | ✅ |
| `Tiesse.pm` | `tiesse.rs` | ✅ |
| `Toshiba.pm` | `toshiba.rs` | ✅ |
| `TpLink.pm` | `tplink.rs` | ✅ |
| `Ubnt.pm` | `ubnt.rs` | ✅ |
| `UPS.pm` | `ups.rs` | ✅ |
| `Voltaire.pm` | `voltaire.rs` | ✅ |
| `Voltronic.pm` | `voltronic.rs` | ✅ |
| `WatchGuard.pm` | `watchguard.rs` | ✅ |
| `WyseThinOS.pm` | `wyse_thinos.rs` | ✅ |
| `Xerox.pm` | `xerox.rs` | ✅ |
| `Zebra.pm` | `zebra.rs` | ✅ |
| `Zyxel.pm` | `zyxel.rs` | ✅ |

## HTTP control server

> **Go:** ⬜ not started (Phase 5, `internal/httpd`).

| Upstream `HTTP/Server/` | Rust `glpi-http` | Status |
| --- | --- | --- |
| `Proxy.pm` | `proxy.rs` | 🟡 ported; `GLPI-Proxy-ID` hop header not forwarded ([proxy.rs](../crates/glpi-http/src/proxy.rs)) |
| `SSL.pm` | `tls.rs` | ✅ |
| `Inventory.pm` | `server.rs` (control endpoints) | ✅ |
| `BasicAuthentication.pm` | — (only IP trust in `trust.rs`) | ⬜ fehlt |
| `SecondaryProxy.pm` | — | ⬜ fehlt |
| `Test.pm` | — | ⬜ minor |
| `ToolBox.pm` | — | 🚫 web GUI, intentionally dropped |

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
