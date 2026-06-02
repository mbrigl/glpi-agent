// SPDX-License-Identifier: GPL-2.0-only

//! The ESX task: connect to a vSphere endpoint, collect its hosts and render
//! them to GLPI inventories.
//!
//! [`EsxTask`] drives the protocol flow over a [`SoapTransport`]
//! (`RetrieveServiceContent` → `Login` → `RetrieveProperties` → `Logout`) and
//! converts each [`HostInfo`] into an [`EsxContent`]. The transport is injected,
//! so the whole flow runs offline against a [`MockTransport`] in tests.
//!
//! Two offline modes mirror the Perl agent:
//!
//! - **dump** ([`EsxTask::collect_hosts`] + [`dump_hosts`]) — fetch the parsed
//!   [`HostInfo`] values and serialize them to JSON for later replay.
//! - **dumpfile** ([`hosts_from_dump`]) — load previously dumped hosts and build
//!   inventories without touching the network.

use glpi_core::error::{AgentError, Result};

use crate::content::{ConversionOptions, EsxContent};
use crate::host::HostInfo;
use crate::parse::parse_hosts;
use crate::soap::{self, parse_service_content, ReqwestTransport, SoapTransport};

/// The GLPI version at which per-VM `operatingsystem`/`ipaddress` reporting was
/// added to the inventory schema (v1.1.36).
const VM_OS_IP_MIN_VERSION: (u32, u32, u32) = (10, 0, 17);

/// Runtime options for an [`EsxTask`].
#[derive(Debug, Clone)]
pub struct EsxOptions {
    /// Accept self-signed / invalid TLS certificates (common for ESXi hosts).
    pub accept_invalid_certs: bool,
    /// GLPI `itemtype` for the submission (default `"Computer"`).
    pub itemtype: String,
    /// Target GLPI version; enables schema features such as VM OS/IP reporting.
    pub glpi_version: Option<String>,
}

impl Default for EsxOptions {
    fn default() -> Self {
        Self {
            accept_invalid_certs: false,
            itemtype: "Computer".to_owned(),
            glpi_version: None,
        }
    }
}

impl EsxOptions {
    /// Derives the inventory [`ConversionOptions`] from the target GLPI version.
    #[must_use]
    pub fn conversion_options(&self) -> ConversionOptions {
        ConversionOptions {
            vm_os_ip: self
                .glpi_version
                .as_deref()
                .is_some_and(|v| version_at_least(v, VM_OS_IP_MIN_VERSION)),
        }
    }
}

/// A single host's inventory: its device id, GLPI itemtype and content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInventory {
    /// The GLPI device id (the host name, else its UUID).
    pub deviceid: String,
    /// The GLPI `itemtype` the content should be submitted as.
    pub itemtype: String,
    /// The collected inventory.
    pub content: EsxContent,
}

/// The ESX inventory task for one vSphere endpoint (ESXi host or vCenter).
#[derive(Debug, Clone)]
pub struct EsxTask {
    host: String,
    user: String,
    password: String,
    options: EsxOptions,
}

impl EsxTask {
    /// Builds a task targeting `host` with the given credentials.
    #[must_use]
    pub fn new(
        host: impl Into<String>,
        user: impl Into<String>,
        password: impl Into<String>,
        options: EsxOptions,
    ) -> Self {
        Self {
            host: host.into(),
            user: user.into(),
            password: password.into(),
            options,
        }
    }

    /// Connects over HTTPS, logs in, retrieves every host and logs out.
    ///
    /// # Errors
    ///
    /// Propagates transport, authentication and protocol errors.
    pub async fn collect_hosts(&self) -> Result<Vec<HostInfo>> {
        let transport = ReqwestTransport::new(&self.host, self.options.accept_invalid_certs)?;
        self.collect_hosts_with(&transport).await
    }

    /// Like [`EsxTask::collect_hosts`] but over an injected transport (tests).
    ///
    /// # Errors
    ///
    /// Propagates transport, authentication and protocol errors.
    pub async fn collect_hosts_with(&self, transport: &dyn SoapTransport) -> Result<Vec<HostInfo>> {
        let content = transport.send(soap::retrieve_service_content()).await?;
        let service = parse_service_content(&content)?;
        tracing::debug!(api = ?service.api_type, product = ?service.full_name, "connected to vSphere");

        let login = transport
            .send(soap::login(
                &service.session_manager,
                &self.user,
                &self.password,
            ))
            .await?;
        if let Some(message) = soap::fault_message(&login) {
            return Err(AgentError::Auth(format!("vSphere login failed: {message}")));
        }

        let properties = transport
            .send(soap::retrieve_properties(
                &service.property_collector,
                &service.root_folder,
            ))
            .await;

        // Always attempt to log out, even if property retrieval failed.
        let _ = transport.send(soap::logout(&service.session_manager)).await;

        let hosts = parse_hosts(&properties?)?;
        tracing::info!(hosts = hosts.len(), "retrieved vSphere hosts");
        Ok(hosts)
    }

    /// Connects and renders every host to a GLPI inventory.
    ///
    /// # Errors
    ///
    /// Propagates the errors of [`EsxTask::collect_hosts`].
    pub async fn run(&self) -> Result<Vec<HostInventory>> {
        let hosts = self.collect_hosts().await?;
        Ok(self.inventories(&hosts))
    }

    /// Renders already-collected hosts to inventories (the dump-replay path).
    #[must_use]
    pub fn inventories(&self, hosts: &[HostInfo]) -> Vec<HostInventory> {
        let conversion = self.options.conversion_options();
        hosts
            .iter()
            .map(|host| HostInventory {
                deviceid: device_id(host),
                itemtype: self.options.itemtype.clone(),
                content: EsxContent::from_host(host, conversion),
            })
            .collect()
    }
}

/// Derives a stable device id for a host: its name, else its UUID, else a
/// fixed fallback so the submission still has an identifier.
fn device_id(host: &HostInfo) -> String {
    host.name
        .clone()
        .or_else(|| host.uuid.clone())
        .unwrap_or_else(|| "esx-host".to_owned())
}

/// Serializes collected hosts to the dump JSON (pretty-printed).
///
/// # Errors
///
/// Returns [`AgentError::Json`] if serialization fails.
pub fn dump_hosts(hosts: &[HostInfo]) -> Result<String> {
    Ok(serde_json::to_string_pretty(hosts)?)
}

/// Loads hosts from dump JSON, accepting either a single host object or an array
/// of hosts.
///
/// # Errors
///
/// Returns [`AgentError::Json`] if the text is neither shape.
pub fn hosts_from_dump(json: &str) -> Result<Vec<HostInfo>> {
    if let Ok(hosts) = serde_json::from_str::<Vec<HostInfo>>(json) {
        return Ok(hosts);
    }
    let host: HostInfo = serde_json::from_str(json)?;
    Ok(vec![host])
}

/// Returns `true` if dotted version string `version` is at least `min`.
///
/// Missing components compare as `0`; trailing build suffixes are ignored.
fn version_at_least(version: &str, min: (u32, u32, u32)) -> bool {
    let mut parts = version
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u32>().unwrap_or(0));
    let parsed = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    parsed >= min
}

#[cfg(test)]
mod tests {
    use super::{hosts_from_dump, version_at_least, EsxOptions, EsxTask};
    use crate::host::HostInfo;
    use crate::soap::MockTransport;

    const SERVICE_CONTENT: &str = "<RetrieveServiceContentResponse xmlns=\"urn:vim25\"><returnval>\
        <rootFolder type=\"Folder\">ha-folder-root</rootFolder>\
        <propertyCollector type=\"PropertyCollector\">ha-pc</propertyCollector>\
        <sessionManager type=\"SessionManager\">ha-sm</sessionManager>\
        <about><apiType>HostAgent</apiType></about></returnval></RetrieveServiceContentResponse>";

    const LOGIN_OK: &str = "<LoginResponse xmlns=\"urn:vim25\"><returnval><key>session-1</key></returnval></LoginResponse>";

    const PROPERTIES: &str = "<RetrievePropertiesResponse xmlns=\"urn:vim25\"><returnval>\
        <obj type=\"HostSystem\">host-1</obj>\
        <propSet><name>config.network.dnsConfig.hostName</name><val xsi:type=\"xsd:string\">esx1.lab</val></propSet>\
        <propSet><name>hardware.memorySize</name><val xsi:type=\"xsd:long\">34359738368</val></propSet>\
        </returnval></RetrievePropertiesResponse>";

    const LOGOUT_OK: &str = "<LogoutResponse xmlns=\"urn:vim25\"></LogoutResponse>";

    fn task() -> EsxTask {
        EsxTask::new("esx.lab", "root", "secret", EsxOptions::default())
    }

    #[tokio::test]
    async fn runs_full_flow_over_mock_transport() {
        let transport = MockTransport::new([
            SERVICE_CONTENT.to_owned(),
            LOGIN_OK.to_owned(),
            PROPERTIES.to_owned(),
            LOGOUT_OK.to_owned(),
        ]);
        let hosts = task().collect_hosts_with(&transport).await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name.as_deref(), Some("esx1.lab"));

        let inventories = task().inventories(&hosts);
        assert_eq!(inventories.len(), 1);
        assert_eq!(inventories[0].deviceid, "esx1.lab");
        assert_eq!(inventories[0].itemtype, "Computer");
        assert_eq!(inventories[0].content.memories[0].capacity, Some(32768));
    }

    #[tokio::test]
    async fn login_fault_surfaces_as_auth_error() {
        let fault = "<soapenv:Body><soapenv:Fault><faultstring>incorrect user name or password</faultstring></soapenv:Fault></soapenv:Body>";
        let transport = MockTransport::new([SERVICE_CONTENT.to_owned(), fault.to_owned()]);
        let err = task().collect_hosts_with(&transport).await.unwrap_err();
        assert!(matches!(err, glpi_core::error::AgentError::Auth(_)));
    }

    #[test]
    fn dump_round_trips_through_dumpfile() {
        let hosts = vec![HostInfo {
            name: Some("esx1".to_owned()),
            memory_bytes: Some(8_589_934_592),
            ..HostInfo::default()
        }];
        let json = super::dump_hosts(&hosts).unwrap();
        assert_eq!(hosts_from_dump(&json).unwrap(), hosts);
    }

    #[test]
    fn dumpfile_accepts_single_object_or_array() {
        let single = hosts_from_dump(r#"{"name":"esx1"}"#).unwrap();
        assert_eq!(single.len(), 1);
        let array = hosts_from_dump(r#"[{"name":"a"},{"name":"b"}]"#).unwrap();
        assert_eq!(array.len(), 2);
    }

    #[test]
    fn vm_os_ip_gated_on_glpi_version() {
        assert!(version_at_least("10.0.17", (10, 0, 17)));
        assert!(version_at_least("10.0.18-build123", (10, 0, 17)));
        assert!(version_at_least("11.0.0", (10, 0, 17)));
        assert!(!version_at_least("10.0.16", (10, 0, 17)));
        assert!(!version_at_least("9.5", (10, 0, 17)));

        let mut options = EsxOptions::default();
        assert!(!options.conversion_options().vm_os_ip);
        options.glpi_version = Some("10.0.17".to_owned());
        assert!(options.conversion_options().vm_os_ip);
    }
}
