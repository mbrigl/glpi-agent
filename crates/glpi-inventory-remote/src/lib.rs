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
//! [`SshCliSession`]), the [`AssetnameSupport`] option, and the Linux command
//! orchestration below. Still to come: russh (SSH mode 2), Perl-on-remote
//! (mode 3), WinRM, `remote-workers` parallelism and delta state files.

pub mod assetname;
pub mod session;
pub mod ssh;
pub mod target;

pub use assetname::AssetnameSupport;
pub use session::{MockSession, RemoteSession};
pub use ssh::SshCliSession;
pub use target::{RemoteScheme, RemoteTarget};

use std::collections::HashSet;

use glpi_core::error::Result;
use glpi_inventory_local as local;
use local::{Content, OperatingSystem};

/// Runs the inventory category command set against a [`RemoteSession`].
///
/// Collection is best-effort: a command that is missing on the remote host (or
/// whose output is empty) simply leaves its section out, mirroring how the
/// local task drops empty sections.
#[derive(Debug, Default, Clone)]
pub struct RemoteInventory {
    /// Disabled category names (lower-cased), from `no-category`.
    disabled: HashSet<String>,
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
            if let Some(text) = try_run(session, "lpstat -p").await {
                content.printers = local::parse_lpstat(&text);
            }
        }

        Ok(content)
    }
}

/// Runs `command`, returning its non-empty stdout or `None` on any failure.
async fn try_run(session: &mut dyn RemoteSession, command: &str) -> Option<String> {
    session.run(command).await.ok().filter(|s| !s.is_empty())
}

/// Reads `path`, returning its non-empty contents or `None` on any failure.
async fn try_read(session: &mut dyn RemoteSession, path: &str) -> Option<String> {
    session.read_file(path).await.ok().filter(|s| !s.is_empty())
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
    async fn rpm_fallback_when_no_dpkg() {
        let mut session = MockSession::new().with_command(
            "rpm -qa --qf '%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\n'",
            "glibc\t2.34-60\tx86_64\n",
        );
        let content = RemoteInventory::new().collect(&mut session).await.unwrap();
        assert_eq!(content.softwares.len(), 1);
        assert_eq!(content.softwares[0].name, "glibc");
    }
}
