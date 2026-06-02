// SPDX-License-Identifier: GPL-2.0-only

//! The vSphere SOAP transport seam and message builders.
//!
//! The vSphere API is a SOAP service rooted at `https://<host>/sdk`. This module
//! provides:
//!
//! - [`SoapTransport`] — the seam the task talks through. The live
//!   [`ReqwestTransport`] posts envelopes over HTTPS; the in-memory
//!   [`MockTransport`] replays canned responses so the protocol flow is tested
//!   without a vCenter.
//! - The pure SOAP envelope wrapper and the `vim25` request-body builders
//!   ([`retrieve_service_content`], [`login`], [`logout`],
//!   [`retrieve_properties`]).
//! - [`ServiceContent`] and its parser, plus [`fault_message`] for surfacing
//!   SOAP faults (notably bad credentials) as typed errors.

use std::sync::Mutex;

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};

/// The `vim25` SOAP body namespace shared by every request.
const VIM25: &str = "urn:vim25";

/// Posts SOAP envelopes to a vSphere endpoint (or replays canned ones in tests).
#[async_trait]
pub trait SoapTransport: Send + Sync {
    /// Sends `envelope` and returns the response body as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Transport`] when the round-trip fails.
    async fn send(&self, envelope: String) -> Result<String>;
}

/// A live HTTPS SOAP transport backed by `reqwest`.
#[derive(Debug)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    endpoint: String,
}

impl ReqwestTransport {
    /// Builds a transport for the SDK endpoint of `host` (`https://<host>/sdk`).
    ///
    /// `accept_invalid_certs` disables TLS verification for self-signed ESXi
    /// certificates (the common lab case).
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Transport`] when the HTTP client cannot be built.
    pub fn new(host: &str, accept_invalid_certs: bool) -> Result<Self> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(accept_invalid_certs)
            .cookie_store(true)
            .build()
            .map_err(|e| AgentError::Transport(format!("building vSphere HTTP client: {e}")))?;
        let endpoint = if host.starts_with("http://") || host.starts_with("https://") {
            format!("{}/sdk", host.trim_end_matches('/'))
        } else {
            format!("https://{host}/sdk")
        };
        Ok(Self { client, endpoint })
    }
}

#[async_trait]
impl SoapTransport for ReqwestTransport {
    async fn send(&self, envelope: String) -> Result<String> {
        let response = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "text/xml; charset=utf-8")
            .header("SOAPAction", VIM25)
            .body(envelope)
            .send()
            .await
            .map_err(|e| {
                AgentError::Transport(format!("vSphere request to {} failed: {e}", self.endpoint))
            })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| AgentError::Transport(format!("reading vSphere response: {e}")))?;
        // vSphere returns HTTP 500 with a SOAP Fault body for application
        // errors; let the caller decode the fault rather than failing on status.
        if status.is_server_error() && fault_message(&text).is_none() {
            return Err(AgentError::Transport(format!(
                "vSphere returned HTTP {status} with no SOAP fault"
            )));
        }
        Ok(text)
    }
}

/// An in-memory transport that returns pre-recorded responses in order.
///
/// Used by tests (and the `--dumpfile` path's protocol tests) to drive the full
/// connect → login → retrieve → logout flow without a network.
#[derive(Debug, Default)]
pub struct MockTransport {
    responses: Mutex<std::collections::VecDeque<String>>,
}

impl MockTransport {
    /// Builds a mock transport that yields `responses` in the given order.
    #[must_use]
    pub fn new(responses: impl IntoIterator<Item = String>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
        }
    }
}

#[async_trait]
impl SoapTransport for MockTransport {
    async fn send(&self, _envelope: String) -> Result<String> {
        self.responses
            .lock()
            .expect("mock transport lock")
            .pop_front()
            .ok_or_else(|| AgentError::Transport("mock transport: no more responses".to_owned()))
    }
}

/// Wraps a `vim25` request `body` in a SOAP 1.1 envelope.
#[must_use]
pub fn envelope(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <soapenv:Envelope xmlns:soapenv=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
         <soapenv:Body>{body}</soapenv:Body></soapenv:Envelope>"
    )
}

/// Builds the `RetrieveServiceContent` request (the API handshake).
#[must_use]
pub fn retrieve_service_content() -> String {
    envelope(&format!(
        "<RetrieveServiceContent xmlns=\"{VIM25}\">\
         <_this type=\"ServiceInstance\">ServiceInstance</_this>\
         </RetrieveServiceContent>"
    ))
}

/// Builds a `Login` request against the given session manager.
#[must_use]
pub fn login(session_manager: &str, user: &str, password: &str) -> String {
    envelope(&format!(
        "<Login xmlns=\"{VIM25}\">\
         <_this type=\"SessionManager\">{sm}</_this>\
         <userName>{u}</userName>\
         <password>{p}</password>\
         </Login>",
        sm = xml_escape(session_manager),
        u = xml_escape(user),
        p = xml_escape(password),
    ))
}

/// Builds a `Logout` request against the given session manager.
#[must_use]
pub fn logout(session_manager: &str) -> String {
    envelope(&format!(
        "<Logout xmlns=\"{VIM25}\"><_this type=\"SessionManager\">{sm}</_this></Logout>",
        sm = xml_escape(session_manager),
    ))
}

/// Builds a `RetrieveProperties` request that walks the whole inventory tree
/// from `property_collector`/`root_folder` and returns the requested
/// `HostSystem` and `VirtualMachine` properties in one call.
///
/// The traversal spec is the classic recursive container walk
/// (folder → datacenter → host/vm folders → compute resource → host) used by
/// the VMware sample clients, so the same request works against both a
/// standalone ESXi host and a vCenter.
#[must_use]
pub fn retrieve_properties(property_collector: &str, root_folder: &str) -> String {
    envelope(&format!(
        "<RetrieveProperties xmlns=\"{VIM25}\">\
         <_this type=\"PropertyCollector\">{pc}</_this>\
         <specSet>\
         {host_spec}\
         {vm_spec}\
         <objectSet>\
         <obj type=\"Folder\">{root}</obj>\
         <skip>false</skip>\
         {traversal}\
         </objectSet>\
         </specSet>\
         </RetrieveProperties>",
        pc = xml_escape(property_collector),
        root = xml_escape(root_folder),
        host_spec = prop_spec("HostSystem", HOST_PROPERTIES),
        vm_spec = prop_spec("VirtualMachine", VM_PROPERTIES),
        traversal = TRAVERSAL_SPEC,
    ))
}

/// The `HostSystem` properties requested for each host.
const HOST_PROPERTIES: &[&str] = &[
    "name",
    "summary.config.name",
    "config.network.dnsConfig.hostName",
    "config.product.fullName",
    "config.product.version",
    "hardware.systemInfo.uuid",
    "hardware.systemInfo.vendor",
    "hardware.systemInfo.model",
    "hardware.systemInfo.serialNumber",
    "hardware.biosInfo.biosVersion",
    "hardware.biosInfo.releaseDate",
    "hardware.memorySize",
    "summary.hardware.cpuModel",
    "summary.hardware.cpuMhz",
    "summary.hardware.numCpuPkgs",
    "summary.hardware.numCpuCores",
    "summary.hardware.numCpuThreads",
    "vm",
];

/// The `VirtualMachine` properties requested for each guest.
const VM_PROPERTIES: &[&str] = &[
    "summary.config.name",
    "summary.config.uuid",
    "summary.config.numCpu",
    "summary.config.memorySizeMB",
    "summary.config.guestFullName",
    "summary.config.annotation",
    "summary.guest.guestFullName",
    "summary.guest.ipAddress",
    "summary.runtime.powerState",
];

/// Builds a `<propSet>` selecting the named properties of `type_name`.
fn prop_spec(type_name: &str, properties: &[&str]) -> String {
    let paths: String = properties
        .iter()
        .map(|p| format!("<pathSet>{p}</pathSet>"))
        .collect();
    format!("<propSet><type>{type_name}</type><all>false</all>{paths}</propSet>")
}

/// The recursive selection set descending the inventory hierarchy.
const TRAVERSAL_SPEC: &str = "\
<selectSet xsi:type=\"TraversalSpec\">\
<name>folderTraversal</name><type>Folder</type><path>childEntity</path><skip>false</skip>\
<selectSet><name>folderTraversal</name></selectSet>\
<selectSet><name>dcHostFolder</name></selectSet>\
<selectSet><name>dcVmFolder</name></selectSet>\
<selectSet><name>crHosts</name></selectSet>\
<selectSet><name>crRp</name></selectSet>\
<selectSet><name>rpVm</name></selectSet>\
</selectSet>\
<selectSet xsi:type=\"TraversalSpec\">\
<name>dcHostFolder</name><type>Datacenter</type><path>hostFolder</path><skip>false</skip>\
<selectSet><name>folderTraversal</name></selectSet>\
</selectSet>\
<selectSet xsi:type=\"TraversalSpec\">\
<name>dcVmFolder</name><type>Datacenter</type><path>vmFolder</path><skip>false</skip>\
<selectSet><name>folderTraversal</name></selectSet>\
</selectSet>\
<selectSet xsi:type=\"TraversalSpec\">\
<name>crHosts</name><type>ComputeResource</type><path>host</path><skip>false</skip>\
</selectSet>\
<selectSet xsi:type=\"TraversalSpec\">\
<name>crRp</name><type>ComputeResource</type><path>resourcePool</path><skip>false</skip>\
<selectSet><name>rpVm</name></selectSet>\
</selectSet>\
<selectSet xsi:type=\"TraversalSpec\">\
<name>rpVm</name><type>ResourcePool</type><path>vm</path><skip>false</skip>\
</selectSet>";

/// The handshake result: the managed-object references the task needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceContent {
    /// The `SessionManager` moref (target of `Login` / `Logout`).
    pub session_manager: String,
    /// The `PropertyCollector` moref (target of `RetrieveProperties`).
    pub property_collector: String,
    /// The inventory `rootFolder` moref (the traversal start).
    pub root_folder: String,
    /// The API type, e.g. `"HostAgent"` (ESXi) or `"VirtualCenter"`.
    pub api_type: Option<String>,
    /// The product full name, e.g. `"VMware ESXi 7.0.3 build-…"`.
    pub full_name: Option<String>,
}

/// Parses a `RetrieveServiceContentResponse` into a [`ServiceContent`].
///
/// # Errors
///
/// Returns [`AgentError::Protocol`] when the mandatory morefs are missing (an
/// unexpected response shape), or [`AgentError::Auth`]/[`AgentError::Protocol`]
/// when the body is a SOAP fault instead.
pub fn parse_service_content(xml: &str) -> Result<ServiceContent> {
    if let Some(message) = fault_message(xml) {
        return Err(fault_error(&message));
    }
    let session_manager = element_text(xml, "sessionManager");
    let property_collector = element_text(xml, "propertyCollector");
    let root_folder = element_text(xml, "rootFolder");
    let (Some(session_manager), Some(property_collector), Some(root_folder)) =
        (session_manager, property_collector, root_folder)
    else {
        return Err(AgentError::Protocol(
            "RetrieveServiceContent response missing required managed-object references".to_owned(),
        ));
    };
    Ok(ServiceContent {
        session_manager,
        property_collector,
        root_folder,
        api_type: element_text(xml, "apiType"),
        full_name: element_text(xml, "fullName"),
    })
}

/// Returns the SOAP `faultstring` text if `xml` is a SOAP fault, else `None`.
#[must_use]
pub fn fault_message(xml: &str) -> Option<String> {
    element_text(xml, "faultstring")
}

/// Classifies a fault message into an [`AgentError`]: a login/permission fault
/// becomes [`AgentError::Auth`], anything else [`AgentError::Protocol`].
fn fault_error(message: &str) -> AgentError {
    let lower = message.to_ascii_lowercase();
    if lower.contains("login") || lower.contains("password") || lower.contains("permission") {
        AgentError::Auth(format!("vSphere login failed: {message}"))
    } else {
        AgentError::Protocol(format!("vSphere fault: {message}"))
    }
}

/// Returns the trimmed text of the first `<local>` element (any namespace
/// prefix), or `None`. Shared by the small fixed-shape response parsers.
#[must_use]
pub fn element_text(xml: &str, local: &str) -> Option<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut capture = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == local.as_bytes() => capture = true,
            Ok(Event::Text(t)) if capture => {
                return t.unescape().ok().map(|s| s.trim().to_owned());
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == local.as_bytes() => capture = false,
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

/// Escapes the five XML special characters for safe inclusion in a request.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{
        envelope, fault_message, login, parse_service_content, retrieve_properties,
        retrieve_service_content, MockTransport, SoapTransport,
    };

    #[test]
    fn envelope_wraps_body() {
        let xml = envelope("<X/>");
        assert!(xml.contains("<soapenv:Body><X/></soapenv:Body>"));
        assert!(xml.contains("xmlns:xsi="));
    }

    #[test]
    fn login_escapes_credentials() {
        let xml = login("ha-sessionmgr", "root", "p&w<d");
        assert!(xml.contains("<userName>root</userName>"));
        assert!(xml.contains("<password>p&amp;w&lt;d</password>"));
        assert!(xml.contains("type=\"SessionManager\">ha-sessionmgr"));
    }

    #[test]
    fn retrieve_properties_requests_host_and_vm_specs() {
        let xml = retrieve_properties("ha-property-collector", "ha-folder-root");
        assert!(xml.contains("<type>HostSystem</type>"));
        assert!(xml.contains("<type>VirtualMachine</type>"));
        assert!(xml.contains("<pathSet>hardware.memorySize</pathSet>"));
        assert!(xml.contains("<pathSet>summary.runtime.powerState</pathSet>"));
        assert!(xml.contains("folderTraversal"));
    }

    #[test]
    fn parses_service_content() {
        let xml = "<RetrieveServiceContentResponse xmlns=\"urn:vim25\"><returnval>\
            <rootFolder type=\"Folder\">ha-folder-root</rootFolder>\
            <propertyCollector type=\"PropertyCollector\">ha-property-collector</propertyCollector>\
            <sessionManager type=\"SessionManager\">ha-sessionmgr</sessionManager>\
            <about><fullName>VMware ESXi 7.0.3</fullName><apiType>HostAgent</apiType></about>\
            </returnval></RetrieveServiceContentResponse>";
        let sc = parse_service_content(xml).unwrap();
        assert_eq!(sc.session_manager, "ha-sessionmgr");
        assert_eq!(sc.property_collector, "ha-property-collector");
        assert_eq!(sc.root_folder, "ha-folder-root");
        assert_eq!(sc.api_type.as_deref(), Some("HostAgent"));
        assert_eq!(sc.full_name.as_deref(), Some("VMware ESXi 7.0.3"));
    }

    #[test]
    fn login_fault_is_auth_error() {
        let fault = "<soapenv:Envelope xmlns:soapenv=\"http://schemas.xmlsoap.org/soap/envelope/\">\
            <soapenv:Body><soapenv:Fault><faultcode>ServerFaultCode</faultcode>\
            <faultstring>Cannot complete login due to an incorrect user name or password.</faultstring>\
            </soapenv:Fault></soapenv:Body></soapenv:Envelope>";
        assert!(fault_message(fault)
            .unwrap()
            .contains("incorrect user name"));
        let err = parse_service_content(fault).unwrap_err();
        assert!(matches!(err, glpi_core::error::AgentError::Auth(_)));
    }

    #[tokio::test]
    async fn mock_transport_yields_responses_in_order() {
        let transport = MockTransport::new(["<a/>".to_owned(), "<b/>".to_owned()]);
        assert_eq!(
            transport.send(retrieve_service_content()).await.unwrap(),
            "<a/>"
        );
        assert_eq!(transport.send(envelope("<x/>")).await.unwrap(), "<b/>");
        assert!(transport.send(envelope("<y/>")).await.is_err());
    }
}
