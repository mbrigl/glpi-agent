// SPDX-License-Identifier: GPL-2.0-only

//! Inventory categories.
//!
//! Each category turns a captured data source (a command's output, a `/proc`
//! or `/sys` file) into a typed payload that becomes part of the inventory
//! [`Content`](crate::content::Content). The parsers are pure and unit-tested
//! against fixtures; the live collectors are thin platform wrappers.
//!
//! Currently available:
//!
//! - [`os`] — operating-system identity (`/etc/os-release` + kernel),
//! - [`cpu`] — physical CPUs (`/proc/cpuinfo`),
//! - [`memory`] — memory modules (`dmidecode -t 17`),
//! - [`software`] — installed packages (dpkg / rpm),
//! - [`network`] — network interfaces (`ip -o link` / `ip -o addr`),
//! - [`hardware`] — BIOS / system / board identity (`dmidecode`),
//! - [`storage`] — disks and optical drives (`lsblk`),
//! - [`process`] — running processes (`ps aux`),
//! - [`pci`] — PCI controllers (`lspci -mm`),
//! - [`usb`] — USB devices (`lsusb`),
//! - [`user`] — logged-in users (`who`),
//! - [`battery`] — batteries (`/sys/class/power_supply`),
//! - [`environment`] — process environment variables,
//! - [`video`] / [`sound`] — display and audio controllers (`lspci`),
//! - [`printer`] — CUPS printers (`lpstat -p`),
//! - [`monitor`] — monitors via EDID (`/sys/class/drm`).

pub mod antivirus;
pub mod battery;
pub mod cpu;
pub mod environment;
pub mod hardware;
pub mod memory;
pub mod monitor;
pub mod network;
pub mod os;
pub mod pci;
pub mod printer;
pub mod process;
pub mod software;
pub mod sound;
pub mod storage;
pub mod usb;
pub mod user;
pub mod video;

pub(crate) mod dmi;

pub use antivirus::{detect_present as detect_antivirus, Antivirus};
pub use battery::{parse_power_supply_uevent, Battery};
pub use cpu::{parse_cpuinfo, Cpu};
pub use environment::{from_vars as env_from_vars, EnvVar};
pub use hardware::{parse_dmi_sysfs, parse_dmidecode_hardware, Bios, Hardware};
pub use memory::{parse_dmidecode_memory, MemoryModule};
pub use monitor::{parse_edid, Monitor};
pub use network::{parse_interfaces, NetworkInterface};
pub use os::{parse_os_release, parse_timezone_name, OperatingSystem, Timezone};
pub use pci::{parse_lspci, Controller};
pub use printer::{
    parse_lpstat, parse_lpstat_devices, parse_printers, serial_from_device_uri, Printer,
};
pub use process::{parse_ps, Process};
pub use software::{parse_packages, Software};
pub use sound::{parse_lspci_sound, Sound};
pub use storage::{parse_lsblk, parse_smartctl_info, SmartInfo, Storage};
pub use usb::{parse_lsusb, UsbDevice};
pub use user::{parse_who, User};
pub use video::{parse_lspci_video, Video};
