<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Upstream module mapping

Maps the upstream **Perl** GLPI agent's modules and features to their Rust
counterparts in this workspace, so upstream fixes and features can be located
and ported deliberately rather than rediscovered each time.

This is the **module/feature** layer of upstream tracking. It pairs with:

- [UPSTREAM.md](../UPSTREAM.md) — the pinned upstream **commit** and version mapping,
- [tests/PARITY.md](../tests/PARITY.md) — the **test** (`t/`) parity map.

**Synced to:** upstream `1.17` line, commit `24fec36` (see [UPSTREAM.md](../UPSTREAM.md)).

**Legend:** ✅ ported · 🟡 partial / best-effort · ⬜ missing · 🚫 intentionally dropped.

> Status reflects *module/section presence*, not field-by-field depth inside a
> ported section. Where a section is ported but some platform's detail fields
> are best-effort, it is marked 🟡 with a note.

## Tasks

| Upstream `Task/` | Rust crate | Status |
| --- | --- | --- |
| `Inventory.pm` | `glpi-inventory-local` | ✅ |
| `NetDiscovery.pm` | `glpi-discovery` | ✅ |
| `NetInventory.pm` | `glpi-discovery` | ✅ |
| `ESX.pm` | `glpi-vsphere` | ✅ |
| `RemoteInventory.pm` | `glpi-inventory-remote` | ✅ |
| `Collect.pm` | `glpi-collect` | ✅ |
| `Deploy.pm` | `glpi-deploy` | ✅ |
| `WakeOnLan.pm` | `glpi-wakeonlan` | ✅ |
| _(no upstream equivalent)_ | `glpi-iec61850` | ➕ Rust addition |

## Local inventory sections

Upstream organises inventory modules by OS (`Task/Inventory/{Generic,Linux,Win32,
MacOS,…}`); here they are mapped by the **GLPI JSON section** the data lands in,
which is how the Rust side ([`glpi-inventory-local`](../crates/glpi-inventory-local/src/categories/),
emitted via [`content.rs`](../crates/glpi-inventory-local/src/content.rs)) is organised.

| GLPI section | Upstream module(s) | Rust | Status |
| --- | --- | --- | --- |
| `bios` / `hardware` | `Generic/Dmidecode`, `*/Bios`, `*/Hardware` | `categories/hardware.rs` (+`dmi.rs`) | ✅ |
| `operatingsystem` | `Generic/OS`, `*/OS` | `categories/os.rs` | ✅ |
| `cpus` | `*/CPU` | `categories/cpu.rs` | ✅ |
| `memories` | `*/Memory` | `categories/memory.rs` | ✅ |
| `softwares` | `Generic/Softwares/*` | `categories/software.rs` | ✅ |
| `networks` | `Generic/Networks`, `*/Networks` | `categories/network.rs` | ✅ |
| `storages` | `Generic/Storages/*` | `categories/storage.rs` | ✅ |
| `processes` | `Generic/Processes` | `categories/process.rs` | ✅ |
| `controllers` | `Win32/Controllers`, PCI controllers | `categories/pci.rs` | 🟡 Linux via `lspci`; Win/macOS detail best-effort |
| `usbdevices` | `Generic/USB` | `categories/usb.rs` | ✅ |
| `users` | `Generic/Users` (logged-in) | `categories/user.rs` | ✅ |
| `batteries` | `Generic/Batteries/*` | `categories/battery.rs` | ✅ |
| `envs` | environment variables | `categories/environment.rs` | ✅ |
| `videos` | `*/Videos` | `categories/video.rs` | ✅ |
| `sounds` | `*/Sounds` | `categories/sound.rs` | ✅ |
| `printers` | `Generic/Printers/*` | `categories/printer.rs` | ✅ |
| `monitors` | `Generic/Screen` (EDID) | `categories/monitor.rs` | ✅ |
| `antivirus` | `{Linux,Win32,MacOS}/AntiVirus/*` | `categories/antivirus.rs` | 🟡 platform coverage varies |
| `drives` | `Generic/Drives` (filesystems) | — | ⬜ fehlt |
| `virtualmachines` | `Generic/Virtualization/*`, `Vmsystem` | — | ⬜ fehlt (local host guests; ESX VMs are in `glpi-vsphere`) |
| `licenseinfos` | `Win32/License`, `MacOS/License` | — | ⬜ fehlt |
| `remote_mgmt` | `Generic/Remote_Mgmt/*`, `Generic/Ipmi` | — | ⬜ fehlt (IPMI/BMC, TeamViewer, AnyDesk, …) |
| `databases_services` | `Generic/Databases/*` | — | ⬜ fehlt |
| `firewall` | `Generic/Firewall`, `*/Firewall` | — | ⬜ fehlt |
| `physical_volumes` / `logical_volumes` | `Linux/LVM` | — | ⬜ fehlt (LVM) |
| `local_users` / `local_groups` | `Generic/Users` (local accounts) | — | ⬜ fehlt |
| `inputs` | `{Win32,Linux}/Inputs` | — | ⬜ fehlt (keyboard/mouse) |
| `ports` | `Win32/Ports` | — | ⬜ fehlt |
| `slots` | `Win32/Slots`, `Generic/Dmidecode` | — | ⬜ fehlt |
| `modems` | `Win32/Modems` | — | ⬜ fehlt |
| `powersupplies` | `MacOS/Psu`, dmidecode | — | ⬜ fehlt |
| `accesslog` | `Inventory/AccessLog` | — | 🚫 minor metadata, dropped |
| `rudder` | `Generic/Rudder` | — | 🚫 Rudder integration, dropped |

## Platform inventory coverage

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

| Area | Rust | Status |
| --- | --- | --- |
| system / interfaces / ip | `mib/{system_mib,if_mib,ip_mib}.rs` | ✅ |
| bridge / LLDP / CDP | `mib/{bridge_mib,lldp_mib,cdp_mib}.rs` | ✅ |
| entity / printer | `mib/{entity_mib,printer_mib}.rs` | ✅ |
| device classification | `mib/device.rs` | ✅ |

## SNMP — vendor `MibSupport`

Upstream ships 80 `MibSupport/` modules (78 device/OS + 2 framework); 69 device
MIBs are ported, **9 are missing**, and the 2 framework modules are infra that
the Rust SNMP core handles differently. This table is generated — see
"Keeping this current".

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

| Upstream layer | Rust (`glpi-core::config`) | Status |
| --- | --- | --- |
| defaults → `agent.cfg` → `conf.d/*.cfg` → env → CLI | `config/{sources,mod,options}.rs` | ✅ |
| Windows registry | — | ⬜ fehlt ([config/mod.rs](../crates/glpi-core/src/config/mod.rs)) |

## Keeping this current

This file is part of the pin-bump checklist in [UPSTREAM.md](../UPSTREAM.md): when
the synced commit moves, reconcile this mapping in the same PR.

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
