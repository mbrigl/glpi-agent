// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-vsphere` — VMware ESX / vCenter inventory (Phase 8).
//!
//! Part of the GLPI Agent Rust workspace (v2.0.0).
//!
//! The crate inventories a VMware vSphere endpoint — a standalone ESXi host or a
//! vCenter — over the SOAP `vim25` API and renders each hypervisor host (plus
//! its virtual machines) as a GLPI computer inventory.
//!
//! # Architecture
//!
//! - [`soap`] — the [`SoapTransport`] seam (live HTTPS via [`ReqwestTransport`],
//!   offline via [`MockTransport`]), the request builders and the
//!   service-content / fault parsers.
//! - [`parse`] — turns a `RetrieveProperties` response into [`HostInfo`] values,
//!   folding each host's VMs in by managed-object reference.
//! - [`host`] — the [`HostInfo`] dump model and the SMBIOS placeholder filter.
//! - [`content`] — the GLPI [`EsxContent`] / [`VirtualMachine`] output and the
//!   tested [`HostInfo`] → inventory conversion (BIOS filter, total-RAM memory
//!   estimate, CPU-package split, VM OS/IP reporting).
//! - [`task`] — [`EsxTask`], the connect → login → retrieve → logout flow, plus
//!   the `--dump` / `--dumpfile` offline modes.
//!
//! # Example (offline)
//!
//! ```
//! use glpi_vsphere::{EsxOptions, EsxTask, hosts_from_dump};
//!
//! let hosts = hosts_from_dump(r#"[{"name":"esx1","memory_bytes":8589934592}]"#).unwrap();
//! let task = EsxTask::new("esx1", "root", "secret", EsxOptions::default());
//! let inventories = task.inventories(&hosts);
//! assert_eq!(inventories[0].deviceid, "esx1");
//! assert_eq!(inventories[0].content.memories[0].capacity, Some(8192));
//! ```

pub mod content;
pub mod host;
pub mod parse;
pub mod soap;
pub mod task;

pub use content::{ConversionOptions, EsxContent, VirtualMachine, VERSION_CLIENT};
pub use host::{clean, HostInfo, HostNic, VmInfo};
pub use parse::parse_hosts;
pub use soap::{MockTransport, ReqwestTransport, ServiceContent, SoapTransport};
pub use task::{dump_hosts, hosts_from_dump, EsxOptions, EsxTask, HostInventory};
