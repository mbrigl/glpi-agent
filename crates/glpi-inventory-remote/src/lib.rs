// SPDX-License-Identifier: GPL-2.0-only

//! `glpi-inventory-remote` — Remote inventory over SSH (and, later, WinRM).
//!
//! Remote inventory reuses the local-inventory parsers unchanged: a
//! [`RemoteSession`] supplies the same command output / file contents the local
//! collectors would read, only fetched from a remote host. [`RemoteInventory`]
//! drives that — it runs the documented command set over the session and feeds
//! each output to the matching pure parser from `glpi-inventory-local`, building
//! the same [`Content`](glpi_inventory_local::Content).
//!
//! Implemented so far (Phase 7): the `ssh://`/`winrm://` target model
//! ([`RemoteTarget`]), the [`RemoteSession`] seam with an offline
//! [`MockSession`], **SSH mode 1** (the command-line `ssh` client,
//! [`SshCliSession`]), **SSH mode 2** (the pure-Rust `russh` transport,
//! [`RusshSession`], behind the default `russh` feature), the
//! [`AssetnameSupport`] option, **mode 3 (`perl`)** — remote `perl` one-liners
//! gated on a capability probe ([`RemoteModes`], richer `Net::CUPS` printers,
//! `Net::Domain` FQDN fallback), **WinRM** (WS-Management + WinRS shell,
//! [`WinRmSession`], behind the default `winrm` feature) and the Linux command
//! orchestration below.

pub mod assetname;
pub mod mode;
#[cfg(feature = "russh")]
pub mod russh;
pub mod session;
pub mod ssh;
pub mod target;
#[cfg(feature = "winrm")]
pub mod winrm;

pub use assetname::AssetnameSupport;
pub use mode::RemoteModes;
#[cfg(feature = "russh")]
pub use russh::{RusshOptions, RusshSession};
pub use session::{MockSession, RemoteSession};
pub use ssh::SshCliSession;
pub use target::{RemoteScheme, RemoteTarget};
#[cfg(feature = "winrm")]
pub use winrm::{WinRmOptions, WinRmSession};

use std::collections::HashSet;

use glpi_core::error::{AgentError, Result};
use glpi_inventory_local as local;
use local::{Content, OperatingSystem, Printer};

/// The `perl -MNet::CUPS` one-liner used in `perl` mode to enumerate printers
/// with their URI, driver and description (richer than `lpstat -p`).
const CUPS_PRINTERS_COMMAND: &str = "perl -MNet::CUPS -e 'map { print \"uri: \".$_->getUri().\"\\nname: \".$_->getName().\"\\ndriver: \".$_->getOptionValue(\"printer-make-and-model\").\"\\ndescription: \".$_->getDescription().\"\\n---\\n\" } Net::CUPS->new->getDestinations()'";

/// Runs the inventory category command set against a [`RemoteSession`].
///
/// Collection is best-effort: a command that is missing on the remote host (or
/// whose output is empty) simply leaves its section out, mirroring how the
/// local task drops empty sections.
#[derive(Debug, Default, Clone)]
pub struct RemoteInventory {
    /// Disabled category names (lower-cased), from `no-category`.
    disabled: HashSet<String>,
    /// Enabled remote modes (notably `perl`).
    modes: RemoteModes,
}

impl RemoteInventory {
    /// Creates a remote-inventory task that collects every category.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Excludes the given categories (GLPI `no-category` names, case-insensitive).
    #[must_use]
    pub fn with_disabled_categories<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.disabled = categories
            .into_iter()
            .map(|c| c.as_ref().to_ascii_lowercase())
            .collect();
        self
    }

    /// Sets the enabled remote modes (e.g. `perl`).
    #[must_use]
    pub fn with_modes(mut self, modes: RemoteModes) -> Self {
        self.modes = modes;
        self
    }

    /// Returns `true` if `category` should be collected.
    #[must_use]
    fn enabled(&self, category: &str) -> bool {
        !self.disabled.contains(category)
    }

    /// Collects the enabled categories from `session` into a [`Content`].
    ///
    /// # Errors
    ///
    /// Currently infallible per-section (failures are swallowed as "section
    /// absent"); returns [`Result`] so future transports can surface a fatal
    /// connection error without an API break.
    pub async fn collect(&self, session: &mut dyn RemoteSession) -> Result<Content> {
        // `perl` mode is only meaningful when the remote host actually has a
        // usable perl interpreter (faithful to the upstream precondition).
        if self.modes.perl() && !session.can_run("perl").await {
            return Err(AgentError::Unsupported(
                "mode perl required but remote host can't run perl".to_owned(),
            ));
        }

        let mut content = Content {
            version_client: Some(local::content::VERSION_CLIENT.to_owned()),
            ..Content::default()
        };

        if self.enabled("os") {
            if let Some(text) = try_read(session, "/etc/os-release").await {
                let os = local::parse_os_release(&text);
                content.operating_system = (os != OperatingSystem::default()).then_some(os);
            }
        }
        if self.enabled("cpu") {
            if let Some(text) = try_read(session, "/proc/cpuinfo").await {
                content.cpus = local::parse_cpuinfo(&text);
            }
        }
        if self.enabled("memory") {
            if let Some(text) = try_run(session, "dmidecode -t 17").await {
                content.memories = local::parse_dmidecode_memory(&text);
            }
        }
        if self.enabled("hardware") || self.enabled("bios") {
            if let Some(text) = try_run(session, "dmidecode").await {
                let (bios, hardware) = local::parse_dmidecode_hardware(&text);
                if self.enabled("bios") && bios != local::Bios::default() {
                    content.bios = Some(bios);
                }
                if self.enabled("hardware") && hardware != local::Hardware::default() {
                    content.hardware = Some(hardware);
                }
            }
        }
        if self.enabled("software") {
            let packages = match try_run(
                session,
                "dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n'",
            )
            .await
            {
                Some(text) => Some(text),
                None => {
                    try_run(
                        session,
                        "rpm -qa --qf '%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\n'",
                    )
                    .await
                }
            };
            if let Some(text) = packages {
                content.softwares = local::parse_packages(&text);
            }
        }
        if self.enabled("network") {
            if let Some(link) = try_run(session, "ip -o link show").await {
                let addr = try_run(session, "ip -o addr show")
                    .await
                    .unwrap_or_default();
                content.networks = local::parse_interfaces(&link, &addr);
            }
        }
        if self.enabled("storage") {
            if let Some(text) =
                try_run(session, "lsblk -dnPb -o NAME,TYPE,SIZE,MODEL,SERIAL,VENDOR").await
            {
                content.storages = local::parse_lsblk(&text);
            }
        }
        if self.enabled("process") {
            if let Some(text) = try_run(session, "ps aux").await {
                content.processes = local::parse_ps(&text);
            }
        }
        if self.enabled("controller") || self.enabled("video") || self.enabled("sound") {
            if let Some(text) = try_run(session, "lspci -mm").await {
                if self.enabled("controller") {
                    content.controllers = local::parse_lspci(&text);
                }
                if self.enabled("video") {
                    content.videos = local::parse_lspci_video(&text);
                }
                if self.enabled("sound") {
                    content.sounds = local::parse_lspci_sound(&text);
                }
            }
        }
        if self.enabled("usb") {
            if let Some(text) = try_run(session, "lsusb").await {
                content.usb_devices = local::parse_lsusb(&text);
            }
        }
        if self.enabled("user") {
            if let Some(text) = try_run(session, "who").await {
                content.users = local::parse_who(&text);
            }
        }
        if self.enabled("printer") {
            // In perl mode, prefer the Net::CUPS enumeration (it also yields the
            // make-and-model); otherwise the perl-free `lpstat -l -p` + `lpstat
            // -v` path still provides URI, serial and description.
            let from_perl = if self.modes.perl() {
                try_run(session, CUPS_PRINTERS_COMMAND)
                    .await
                    .map(|text| parse_cups_printers(&text))
                    .filter(|p| !p.is_empty())
            } else {
                None
            };
            content.printers = match from_perl {
                Some(printers) => printers,
                None => {
                    let status = try_run(session, "lpstat -l -p").await.unwrap_or_default();
                    let devices = try_run(session, "lpstat -v").await.unwrap_or_default();
                    local::parse_printers(&status, &devices)
                }
            };
        }

        Ok(content)
    }

    /// Collects a Windows host's inventory over a [`RemoteSession`] (typically
    /// WinRM), running the same PowerShell `Get-CimInstance … | ConvertTo-Json`
    /// queries the local Windows collectors use and feeding their outputs to the
    /// shared `parse_win_*` parsers.
    ///
    /// Best-effort per section, like [`collect`](Self::collect): a query that
    /// returns nothing simply leaves its section out.
    ///
    /// # Errors
    ///
    /// Returns [`Result`] for symmetry with [`collect`](Self::collect); failures
    /// are currently swallowed per-section.
    pub async fn collect_windows(&self, session: &mut dyn RemoteSession) -> Result<Content> {
        use local::categories as cat;

        let mut content = Content {
            version_client: Some(local::content::VERSION_CLIENT.to_owned()),
            ..Content::default()
        };

        if self.enabled("os") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,OSArchitecture | ConvertTo-Json -Compress").await {
                content.operating_system = cat::os::parse_win_os(&json);
            }
        }
        if self.enabled("cpu") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_Processor | Select-Object Name,Manufacturer,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed | ConvertTo-Json -Compress").await {
                content.cpus = cat::cpu::parse_win_cpus(&json);
            }
        }
        if self.enabled("memory") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_PhysicalMemory | Select-Object Capacity,Speed,Manufacturer,PartNumber,SerialNumber,DeviceLocator,SMBIOSMemoryType | ConvertTo-Json -Compress").await {
                content.memories = cat::memory::parse_win_memory(&json);
            }
        }
        if self.enabled("bios") || self.enabled("hardware") {
            let script = "$b=Get-CimInstance Win32_BIOS; $c=Get-CimInstance Win32_ComputerSystem; $p=Get-CimInstance Win32_ComputerSystemProduct; $m=Get-CimInstance Win32_BaseBoard; [pscustomobject]@{BiosManufacturer=$b.Manufacturer;BiosVersion=$b.SMBIOSBIOSVersion;BiosDate=$b.ReleaseDate;SystemSerial=$b.SerialNumber;SystemManufacturer=$c.Manufacturer;SystemModel=$c.Model;Name=$c.Name;UUID=$p.UUID;BoardManufacturer=$m.Manufacturer;BoardModel=$m.Product;BoardSerial=$m.SerialNumber} | ConvertTo-Json -Compress";
            if let Some(json) = ps(session, script).await {
                let (bios, hardware) = cat::hardware::parse_win_hardware(&json);
                if self.enabled("bios") && bios != local::Bios::default() {
                    content.bios = Some(bios);
                }
                if self.enabled("hardware") && hardware != local::Hardware::default() {
                    content.hardware = Some(hardware);
                }
            }
        }
        if self.enabled("storage") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_DiskDrive | Select-Object Model,Size,SerialNumber,MediaType,FirmwareRevision,Manufacturer | ConvertTo-Json -Compress").await {
                content.storages = cat::storage::parse_win_storage(&json);
            }
        }
        if self.enabled("software") {
            let script = "$paths=@('HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*','HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*','HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*'); Get-ItemProperty $paths -ErrorAction SilentlyContinue | Where-Object {$_.DisplayName} | Select-Object DisplayName,DisplayVersion,Publisher | ConvertTo-Json -Compress";
            if let Some(json) = ps(session, script).await {
                content.softwares = cat::software::parse_win_software(&json);
            }
            if let Some(json) = ps(
                session,
                "Get-AppxPackage | Select-Object Name,Version | ConvertTo-Json -Compress",
            )
            .await
            {
                content
                    .softwares
                    .extend(cat::software::parse_win_appx(&json));
            }
        }
        if self.enabled("network") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_NetworkAdapterConfiguration | Where-Object {$_.MACAddress} | Select-Object Description,MACAddress,IPAddress | ConvertTo-Json -Compress").await {
                content.networks = cat::network::parse_win_network(&json);
            }
        }
        if self.enabled("process") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_Process | Select-Object ProcessId,Name,CommandLine,VirtualSize | ConvertTo-Json -Compress").await {
                content.processes = cat::process::parse_win_processes(&json);
            }
        }
        if self.enabled("user") {
            if let Some(text) = ps(session, "(Get-CimInstance Win32_ComputerSystem).UserName").await
            {
                content.users = cat::user::parse_win_username(&text);
            }
        }
        if self.enabled("printer") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_Printer | Select-Object Name,DriverName,PortName,Comment | ConvertTo-Json -Compress").await {
                content.printers = cat::printer::parse_win_printers(&json);
            }
        }
        if self.enabled("video") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterCompatibility | ConvertTo-Json -Compress").await {
                content.videos = cat::video::parse_win_video(&json);
            }
        }
        if self.enabled("sound") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_SoundDevice | Select-Object Name,Manufacturer | ConvertTo-Json -Compress").await {
                content.sounds = cat::sound::parse_win_sound(&json);
            }
        }
        if self.enabled("usb") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_PnPEntity | Where-Object {$_.PNPDeviceID -like 'USB\\VID_*'} | Select-Object Name,PNPDeviceID | ConvertTo-Json -Compress").await {
                content.usb_devices = cat::usb::parse_win_usb(&json);
            }
        }
        if self.enabled("battery") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_Battery | Select-Object Name,DesignVoltage,DesignCapacity,Chemistry | ConvertTo-Json -Compress").await {
                content.batteries = cat::battery::parse_win_batteries(&json);
            }
        }
        if self.enabled("controller") {
            if let Some(json) = ps(session, "Get-CimInstance Win32_PnPEntity | Where-Object {$_.PNPDeviceID -like 'PCI\\*'} | Select-Object Name,Manufacturer,PNPClass | ConvertTo-Json -Compress").await {
                content.controllers = cat::pci::parse_win_controllers(&json);
            }
        }
        if self.enabled("monitor") {
            let script = "Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Enum\\DISPLAY' -Recurse -ErrorAction SilentlyContinue | Where-Object {$_.Property -contains 'EDID'} | ForEach-Object {[pscustomobject]@{EDID=(Get-ItemProperty -Path $_.PSPath -Name EDID).EDID}} | ConvertTo-Json -Compress -Depth 4";
            if let Some(json) = ps(session, script).await {
                content.monitors = cat::monitor::parse_win_monitors(&json);
            }
        }
        if self.enabled("antivirus") {
            if let Some(json) = ps(session, "Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct | Select-Object displayName | ConvertTo-Json -Compress").await {
                content.antivirus = cat::antivirus::parse_win_antivirus(&json);
            }
        }

        Ok(content)
    }
}

/// Runs a PowerShell `script` over the session (wrapping it for the remote
/// shell) and returns its non-empty output.
async fn ps(session: &mut dyn RemoteSession, script: &str) -> Option<String> {
    let command = format!("powershell -NoProfile -NonInteractive -Command \"{script}\"");
    try_run(session, &command).await
}

/// Runs `command`, returning its non-empty stdout or `None` on any failure.
async fn try_run(session: &mut dyn RemoteSession, command: &str) -> Option<String> {
    session.run(command).await.ok().filter(|s| !s.is_empty())
}

/// Reads `path`, returning its non-empty contents or `None` on any failure.
async fn try_read(session: &mut dyn RemoteSession, path: &str) -> Option<String> {
    session.read_file(path).await.ok().filter(|s| !s.is_empty())
}

/// Parses the `perl -MNet::CUPS` output ([`CUPS_PRINTERS_COMMAND`]) into
/// printers. Records are `key: value` lines terminated by a `---` separator;
/// the serial is extracted from a `serial=`/`uuid=` parameter in the device URI.
fn parse_cups_printers(text: &str) -> Vec<Printer> {
    let mut printers = Vec::new();
    let mut current = Printer::default();
    let mut have_fields = false;
    for line in text.lines() {
        if line.trim() == "---" {
            if have_fields && !current.name.is_empty() {
                printers.push(std::mem::take(&mut current));
            } else {
                current = Printer::default();
            }
            have_fields = false;
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        have_fields = true;
        match key.trim() {
            "name" => current.name = value.to_owned(),
            "description" => current.description = Some(value.to_owned()),
            "driver" => current.driver = Some(value.to_owned()),
            "uri" => {
                current.serial = local::serial_from_device_uri(value);
                current.port = Some(value.to_owned());
            }
            _ => {}
        }
    }
    if have_fields && !current.name.is_empty() {
        printers.push(current);
    }
    printers
}

#[cfg(test)]
mod tests {
    use super::{MockSession, RemoteInventory};

    fn linux_host() -> MockSession {
        MockSession::new()
            .with_file(
                "/etc/os-release",
                "NAME=\"Ubuntu\"\nVERSION_ID=\"22.04\"\nID=ubuntu\n",
            )
            .with_file(
                "/proc/cpuinfo",
                "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Xeon(R) @ 2.10GHz\nphysical id\t: 0\ncpu cores\t: 4\n\n",
            )
            .with_command(
                "ps aux",
                "USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND\nroot 1 0.0 0.1 1000 200 ? Ss 00:00 0:01 /sbin/init\n",
            )
            .with_command("who", "alice pts/0 2024-01-01 09:00 (10.0.0.9)\n")
            .with_command(
                "dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n'",
                "bash\t5.1-6\tamd64\nopenssl\t3.0.2\tamd64\n",
            )
    }

    #[tokio::test]
    async fn collects_sections_via_session() {
        let mut session = linux_host();
        let content = RemoteInventory::new().collect(&mut session).await.unwrap();

        assert_eq!(
            content.operating_system.as_ref().unwrap().name.as_deref(),
            Some("Ubuntu")
        );
        assert_eq!(content.cpus.len(), 1);
        assert_eq!(content.cpus[0].cores, Some(4));
        assert_eq!(content.softwares.len(), 2);
        assert_eq!(content.processes.len(), 1);
        assert_eq!(content.users.len(), 1);
        // Unmapped commands (lsusb, lspci, …) just leave their sections empty.
        assert!(content.usb_devices.is_empty());
        assert_eq!(
            content.version_client.as_deref(),
            Some(glpi_inventory_local::content::VERSION_CLIENT)
        );
    }

    #[tokio::test]
    async fn honours_disabled_categories() {
        let mut session = linux_host();
        let content = RemoteInventory::new()
            .with_disabled_categories(["software", "cpu"])
            .collect(&mut session)
            .await
            .unwrap();
        assert!(content.softwares.is_empty());
        assert!(content.cpus.is_empty());
        // A still-enabled category is unaffected.
        assert_eq!(content.users.len(), 1);
    }

    #[tokio::test]
    async fn perl_mode_requires_remote_perl() {
        // perl mode but the host can't run perl -> hard error.
        let mut session = linux_host();
        let err = RemoteInventory::new()
            .with_modes(super::RemoteModes::parse("perl"))
            .collect(&mut session)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("perl"));
    }

    #[tokio::test]
    async fn perl_mode_uses_net_cups_for_printers() {
        let cups = "uri: ipp://printer.local/ipp/print?serial=ABC123\n\
                    name: Reception\n\
                    driver: HP LaserJet\n\
                    description: Reception desk\n\
                    ---\n";
        let mut session = linux_host()
            .with_program("perl")
            .with_command(super::CUPS_PRINTERS_COMMAND, cups);
        let content = RemoteInventory::new()
            .with_modes(super::RemoteModes::parse("ssh_perl"))
            .collect(&mut session)
            .await
            .unwrap();
        assert_eq!(content.printers.len(), 1);
        let printer = &content.printers[0];
        assert_eq!(printer.name, "Reception");
        assert_eq!(printer.driver.as_deref(), Some("HP LaserJet"));
        assert_eq!(printer.description.as_deref(), Some("Reception desk"));
        assert_eq!(printer.serial.as_deref(), Some("ABC123"));
        assert_eq!(
            printer.port.as_deref(),
            Some("ipp://printer.local/ipp/print?serial=ABC123")
        );
    }

    #[test]
    fn parse_cups_printers_handles_multiple_blocks() {
        let text = "uri: socket://10.0.0.7\nname: A\n---\nuri: usb://x?serial=SN9\nname: B\n---\n";
        let printers = super::parse_cups_printers(text);
        assert_eq!(printers.len(), 2);
        assert_eq!(printers[0].name, "A");
        assert_eq!(printers[1].serial.as_deref(), Some("SN9"));
    }

    #[tokio::test]
    async fn rpm_fallback_when_no_dpkg() {
        let mut session = MockSession::new().with_command(
            "rpm -qa --qf '%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\n'",
            "glibc\t2.34-60\tx86_64\n",
        );
        let content = RemoteInventory::new().collect(&mut session).await.unwrap();
        assert_eq!(content.softwares.len(), 1);
        assert_eq!(content.softwares[0].name, "glibc");
    }

    /// Wraps a PowerShell script the way [`super::ps`] does, for the mock map.
    fn win(script: &str) -> String {
        format!("powershell -NoProfile -NonInteractive -Command \"{script}\"")
    }

    #[tokio::test]
    async fn collects_windows_sections_via_session() {
        let mut session = MockSession::new()
            .with_command(
                &win("Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,OSArchitecture | ConvertTo-Json -Compress"),
                r#"{"Caption":"Microsoft Windows 11 Pro","Version":"10.0.22631","OSArchitecture":"64-bit"}"#,
            )
            .with_command(
                &win("Get-CimInstance Win32_Processor | Select-Object Name,Manufacturer,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed | ConvertTo-Json -Compress"),
                r#"{"Name":"Intel(R) Core(TM) i7","NumberOfCores":6,"NumberOfLogicalProcessors":12,"MaxClockSpeed":2600}"#,
            )
            .with_command(
                &win("Get-AppxPackage | Select-Object Name,Version | ConvertTo-Json -Compress"),
                r#"[{"Name":"Microsoft.WindowsCalculator","Version":"11.0"}]"#,
            );

        let content = RemoteInventory::new()
            .collect_windows(&mut session)
            .await
            .unwrap();

        assert_eq!(
            content.operating_system.as_ref().unwrap().name.as_deref(),
            Some("Microsoft Windows 11 Pro")
        );
        assert_eq!(content.cpus.len(), 1);
        assert_eq!(content.cpus[0].cores, Some(6));
        // The registry query is unmapped (empty); the Store package still lands.
        assert_eq!(content.softwares.len(), 1);
        assert_eq!(content.softwares[0].name, "Microsoft.WindowsCalculator");
        // Unmapped sections stay empty.
        assert!(content.networks.is_empty());
    }

    #[tokio::test]
    async fn windows_honours_disabled_categories() {
        let mut session = MockSession::new().with_command(
            &win("Get-CimInstance Win32_Processor | Select-Object Name,Manufacturer,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed | ConvertTo-Json -Compress"),
            r#"{"Name":"x","NumberOfCores":4}"#,
        );
        let content = RemoteInventory::new()
            .with_disabled_categories(["cpu"])
            .collect_windows(&mut session)
            .await
            .unwrap();
        assert!(content.cpus.is_empty());
    }
}
