// SPDX-License-Identifier: GPL-2.0-only

//! WinRM transport (WS-Management + WinRS shell).
//!
//! [`WinRmSession`] runs commands on a Windows host through the WS-Management
//! protocol: it opens a WinRS shell, runs a command, drains its output streams
//! and reports the exit status — implementing the same [`RemoteSession`] seam as
//! the SSH transports. Authentication is HTTP Basic (use HTTPS, or enable
//! `AllowUnencrypted` for plain HTTP); NTLM/Negotiate and Kerberos are a
//! follow-up.
//!
//! The SOAP envelope builders and the response parsers are pure and
//! unit-tested; only the HTTP round-trip needs a live host.
//!
//! Note: the inventory orchestrator's command set is Linux-specific, so a full
//! Windows inventory additionally needs the Windows collection commands (Phase
//! 6b). This module provides the transport and ad-hoc command execution.

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use glpi_core::error::{AgentError, Result};

use crate::session::RemoteSession;
use crate::target::RemoteTarget;

// WS-Management / WinRS constants.
const NS_SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const NS_WSA: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing";
const NS_WSMAN: &str = "http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd";
const NS_SHELL: &str = "http://schemas.microsoft.com/wbem/wsman/1/windows/shell";
const RESOURCE_CMD: &str = "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd";
const ANONYMOUS: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous";
const ACTION_CREATE: &str = "http://schemas.xmlsoap.org/ws/2004/09/transfer/Create";
const ACTION_DELETE: &str = "http://schemas.xmlsoap.org/ws/2004/09/transfer/Delete";
const ACTION_COMMAND: &str = "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Command";
const ACTION_RECEIVE: &str = "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Receive";
const ACTION_SIGNAL: &str = "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Signal";
const SIGNAL_TERMINATE: &str =
    "http://schemas.microsoft.com/wbem/wsman/1/windows/shell/signal/terminate";

/// Connection options for [`WinRmSession`].
#[derive(Debug, Clone)]
pub struct WinRmOptions {
    /// Per-operation timeout (WS-Man `OperationTimeout`, ISO-8601 seconds).
    pub operation_timeout_secs: u64,
    /// Accept invalid/self-signed TLS certificates (HTTPS only).
    pub accept_invalid_certs: bool,
}

impl Default for WinRmOptions {
    fn default() -> Self {
        Self {
            operation_timeout_secs: 60,
            accept_invalid_certs: false,
        }
    }
}

/// A [`RemoteSession`] backed by a WinRM (WS-Management) endpoint.
#[cfg(feature = "winrm")]
pub struct WinRmSession {
    client: reqwest::Client,
    endpoint: String,
    user: String,
    password: String,
    operation_timeout_secs: u64,
    shell_id: String,
}

#[cfg(feature = "winrm")]
impl WinRmSession {
    /// Connects to `target` and opens a WinRS shell.
    ///
    /// The endpoint is `http(s)://host:port/wsman` (HTTPS when the port is 5986
    /// or the target carries `?ssl=1`, else HTTP on 5985 by default).
    ///
    /// # Errors
    ///
    /// [`AgentError::Auth`] when credentials are missing or rejected;
    /// [`AgentError::Task`] on transport/protocol failures.
    pub async fn connect(target: &RemoteTarget, options: &WinRmOptions) -> Result<Self> {
        let user = target
            .user
            .clone()
            .ok_or_else(|| AgentError::Auth("WinRM requires a user".to_owned()))?;
        let password = target
            .password
            .clone()
            .ok_or_else(|| AgentError::Auth("WinRM requires a password (Basic auth)".to_owned()))?;

        let https =
            target.option("ssl").is_some_and(|v| v != "0") || matches!(target.port, Some(5986));
        let port = target.port.unwrap_or(if https { 5986 } else { 5985 });
        let scheme = if https { "https" } else { "http" };
        let endpoint = format!("{scheme}://{}:{port}/wsman", target.host);

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(options.accept_invalid_certs)
            .build()
            .map_err(|e| AgentError::Task(format!("building WinRM HTTP client: {e}")))?;

        let mut session = Self {
            client,
            endpoint,
            user,
            password,
            operation_timeout_secs: options.operation_timeout_secs,
            shell_id: String::new(),
        };
        let create = create_shell_envelope(&session.endpoint, session.operation_timeout_secs);
        let response = session.post(create).await?;
        session.shell_id = parse_shell_id(&response)
            .ok_or_else(|| AgentError::Task("WinRM Create returned no ShellId".to_owned()))?;
        Ok(session)
    }

    /// Closes the WinRS shell (best-effort).
    ///
    /// # Errors
    ///
    /// Propagates a transport failure from the `Delete` request.
    pub async fn close(&mut self) -> Result<()> {
        let envelope =
            delete_shell_envelope(&self.endpoint, &self.shell_id, self.operation_timeout_secs);
        self.post(envelope).await.map(|_| ())
    }

    /// POSTs a SOAP envelope and returns the response body.
    async fn post(&self, body: String) -> Result<String> {
        let response = self
            .client
            .post(&self.endpoint)
            .basic_auth(&self.user, Some(&self.password))
            .header("Content-Type", "application/soap+xml;charset=UTF-8")
            .body(body)
            .send()
            .await
            .map_err(|e| {
                AgentError::Task(format!("WinRM request to {} failed: {e}", self.endpoint))
            })?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AgentError::Auth(format!(
                "WinRM authentication rejected by {}",
                self.endpoint
            )));
        }
        let text = response
            .text()
            .await
            .map_err(|e| AgentError::Task(format!("reading WinRM response: {e}")))?;
        if !status.is_success() {
            return Err(AgentError::Task(format!(
                "WinRM request failed ({status}): {}",
                text.trim()
            )));
        }
        Ok(text)
    }
}

#[cfg(feature = "winrm")]
#[async_trait]
impl RemoteSession for WinRmSession {
    async fn run(&mut self, command: &str) -> Result<String> {
        let command_envelope = command_envelope(
            &self.endpoint,
            &self.shell_id,
            command,
            self.operation_timeout_secs,
        );
        let response = self.post(command_envelope).await?;
        let command_id = parse_command_id(&response)
            .ok_or_else(|| AgentError::Task("WinRM Command returned no CommandId".to_owned()))?;

        let mut stdout = Vec::new();
        // Drain the output streams until the command reports Done.
        let exit_code = loop {
            let receive = receive_envelope(
                &self.endpoint,
                &self.shell_id,
                &command_id,
                self.operation_timeout_secs,
            );
            let response = self.post(receive).await?;
            let output = parse_receive(&response)?;
            stdout.extend_from_slice(&output.stdout);
            if output.done {
                break output.exit_code;
            }
        };

        // Terminate the command (best-effort).
        let signal = signal_envelope(
            &self.endpoint,
            &self.shell_id,
            &command_id,
            self.operation_timeout_secs,
        );
        let _ = self.post(signal).await;

        match exit_code {
            Some(0) | None => Ok(String::from_utf8_lossy(&stdout).into_owned()),
            Some(code) => Err(AgentError::Task(format!(
                "remote command exited with status {code}"
            ))),
        }
    }
}

// --- Pure SOAP envelope builders ---------------------------------------------

/// Builds the common WS-Man SOAP header for `action` on the cmd resource, with
/// an optional `ShellId` selector.
fn header(endpoint: &str, action: &str, shell_id: Option<&str>, timeout_secs: u64) -> String {
    let message_id = format!("uuid:{}", uuid::Uuid::new_v4());
    let selector = shell_id.map_or(String::new(), |id| {
        format!(
            "<w:SelectorSet><w:Selector Name=\"ShellId\">{}</w:Selector></w:SelectorSet>",
            xml_escape(id)
        )
    });
    format!(
        "<s:Header>\
<a:To>{endpoint}</a:To>\
<w:ResourceURI s:mustUnderstand=\"true\">{RESOURCE_CMD}</w:ResourceURI>\
<a:ReplyTo><a:Address s:mustUnderstand=\"true\">{ANONYMOUS}</a:Address></a:ReplyTo>\
<w:MaxEnvelopeSize s:mustUnderstand=\"true\">153600</w:MaxEnvelopeSize>\
<a:MessageID>{message_id}</a:MessageID>\
<w:Locale xml:lang=\"en-US\" s:mustUnderstand=\"false\"/>\
<w:OperationTimeout>PT{timeout_secs}S</w:OperationTimeout>\
<a:Action s:mustUnderstand=\"true\">{action}</a:Action>\
{selector}\
</s:Header>"
    )
}

/// Wraps `header` + `body` in a SOAP envelope with the WS-Man namespaces.
fn envelope(header: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<s:Envelope xmlns:s=\"{NS_SOAP}\" xmlns:a=\"{NS_WSA}\" xmlns:w=\"{NS_WSMAN}\" xmlns:rsp=\"{NS_SHELL}\">\
{header}<s:Body>{body}</s:Body></s:Envelope>"
    )
}

/// `Create` a WinRS shell with stdin / stdout+stderr streams.
fn create_shell_envelope(endpoint: &str, timeout_secs: u64) -> String {
    let body = "<rsp:Shell><rsp:InputStreams>stdin</rsp:InputStreams>\
<rsp:OutputStreams>stdout stderr</rsp:OutputStreams></rsp:Shell>";
    envelope(&header(endpoint, ACTION_CREATE, None, timeout_secs), body)
}

/// `Command` to run `command` in the shell (via the default `cmd.exe`).
fn command_envelope(endpoint: &str, shell_id: &str, command: &str, timeout_secs: u64) -> String {
    let body = format!(
        "<rsp:CommandLine><rsp:Command>{}</rsp:Command></rsp:CommandLine>",
        xml_escape(command)
    );
    envelope(
        &header(endpoint, ACTION_COMMAND, Some(shell_id), timeout_secs),
        &body,
    )
}

/// `Receive` to pull stdout/stderr for `command_id`.
fn receive_envelope(endpoint: &str, shell_id: &str, command_id: &str, timeout_secs: u64) -> String {
    let body = format!(
        "<rsp:Receive><rsp:DesiredStream CommandId=\"{}\">stdout stderr</rsp:DesiredStream></rsp:Receive>",
        xml_escape(command_id)
    );
    envelope(
        &header(endpoint, ACTION_RECEIVE, Some(shell_id), timeout_secs),
        &body,
    )
}

/// `Signal` to terminate `command_id`.
fn signal_envelope(endpoint: &str, shell_id: &str, command_id: &str, timeout_secs: u64) -> String {
    let body = format!(
        "<rsp:Signal CommandId=\"{}\"><rsp:Code>{SIGNAL_TERMINATE}</rsp:Code></rsp:Signal>",
        xml_escape(command_id)
    );
    envelope(
        &header(endpoint, ACTION_SIGNAL, Some(shell_id), timeout_secs),
        &body,
    )
}

/// `Delete` to close the shell.
fn delete_shell_envelope(endpoint: &str, shell_id: &str, timeout_secs: u64) -> String {
    envelope(
        &header(endpoint, ACTION_DELETE, Some(shell_id), timeout_secs),
        "",
    )
}

/// Minimal XML text/attribute escaping.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// --- Pure response parsers ---------------------------------------------------

/// The decoded result of one `Receive` response.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReceiveOutput {
    /// Decoded stdout bytes from this batch.
    pub stdout: Vec<u8>,
    /// Decoded stderr bytes from this batch.
    pub stderr: Vec<u8>,
    /// `true` once the command's `CommandState` reaches `Done`.
    pub done: bool,
    /// Exit code, present once `done`.
    pub exit_code: Option<i32>,
}

/// Returns the text of the first element whose local name is `local`.
fn first_element_text(xml: &str, local: &[u8]) -> Option<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut in_target = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == local => in_target = true,
            Ok(Event::Text(e)) if in_target => return e.unescape().ok().map(|t| t.into_owned()),
            Ok(Event::End(e)) if e.local_name().as_ref() == local => in_target = false,
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

/// Extracts the `ShellId` from a `Create` response.
#[must_use]
pub fn parse_shell_id(xml: &str) -> Option<String> {
    first_element_text(xml, b"ShellId")
}

/// Extracts the `CommandId` from a `Command` response.
#[must_use]
pub fn parse_command_id(xml: &str) -> Option<String> {
    first_element_text(xml, b"CommandId")
}

/// Parses a `Receive` response: decodes the `stdout`/`stderr` streams and reads
/// the command's `Done` state and exit code.
///
/// # Errors
///
/// Returns [`AgentError::Protocol`] if a stream payload is not valid base64.
pub fn parse_receive(xml: &str) -> Result<ReceiveOutput> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut output = ReceiveOutput::default();
    // Track which stream the current text belongs to ("stdout"/"stderr"), and
    // whether we are inside an ExitCode element.
    let mut current_stream: Option<String> = None;
    let mut in_exit_code = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"Stream" => current_stream = attr_value(&e, b"Name"),
                b"CommandState" => {
                    if let Some(state) = attr_value(&e, b"State") {
                        output.done |= state.ends_with("/Done") || state.ends_with("Done");
                    }
                }
                b"ExitCode" => in_exit_code = true,
                _ => {}
            },
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"CommandState" => {
                if let Some(state) = attr_value(&e, b"State") {
                    output.done |= state.ends_with("Done");
                }
            }
            Ok(Event::Text(e)) => {
                let text = e
                    .unescape()
                    .map_err(|err| AgentError::Protocol(format!("WinRM response text: {err}")))?;
                if in_exit_code {
                    output.exit_code = text.trim().parse().ok();
                } else if let Some(stream) = &current_stream {
                    let decoded = BASE64.decode(text.trim()).map_err(|err| {
                        AgentError::Protocol(format!("WinRM stream base64: {err}"))
                    })?;
                    match stream.as_str() {
                        "stdout" => output.stdout.extend_from_slice(&decoded),
                        "stderr" => output.stderr.extend_from_slice(&decoded),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"Stream" => current_stream = None,
                b"ExitCode" => in_exit_code = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(AgentError::Protocol(format!(
                    "parsing WinRM response: {err}"
                )))
            }
            _ => {}
        }
    }
    Ok(output)
}

/// Returns the value of attribute `name` on a start/empty element.
fn attr_value(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|attr| {
        (attr.key.local_name().as_ref() == name)
            .then(|| attr.unescape_value().ok().map(|v| v.into_owned()))
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        command_envelope, create_shell_envelope, parse_command_id, parse_receive, parse_shell_id,
        xml_escape, RESOURCE_CMD,
    };
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;

    #[test]
    fn create_envelope_is_well_formed() {
        let xml = create_shell_envelope("http://win:5985/wsman", 60);
        assert!(xml.contains("<a:To>http://win:5985/wsman</a:To>"));
        assert!(xml.contains(RESOURCE_CMD));
        assert!(xml.contains("PT60S"));
        assert!(xml.contains("<rsp:OutputStreams>stdout stderr</rsp:OutputStreams>"));
        assert!(xml.contains("uuid:"));
    }

    #[test]
    fn command_envelope_carries_shell_selector_and_escapes() {
        let xml = command_envelope("http://win:5985/wsman", "SHELL-1", "echo a & b", 60);
        assert!(xml.contains("<w:Selector Name=\"ShellId\">SHELL-1</w:Selector>"));
        assert!(xml.contains("<rsp:Command>echo a &amp; b</rsp:Command>"));
    }

    #[test]
    fn xml_escape_handles_specials() {
        assert_eq!(xml_escape("a<b>&\"c"), "a&lt;b&gt;&amp;&quot;c");
    }

    #[test]
    fn parses_shell_and_command_ids() {
        let create = r#"<s:Envelope xmlns:s="x" xmlns:rsp="y">
            <s:Body><rsp:Shell><rsp:ShellId>SH-42</rsp:ShellId></rsp:Shell></s:Body></s:Envelope>"#;
        assert_eq!(parse_shell_id(create).as_deref(), Some("SH-42"));
        let command = r#"<rsp:CommandResponse xmlns:rsp="y"><rsp:CommandId>CMD-7</rsp:CommandId></rsp:CommandResponse>"#;
        assert_eq!(parse_command_id(command).as_deref(), Some("CMD-7"));
    }

    #[test]
    fn parses_receive_streams_and_exit_code() {
        let stdout_b64 = BASE64.encode("WIN-HOST\r\n");
        let xml = format!(
            r#"<s:Envelope xmlns:s="x" xmlns:rsp="y">
              <s:Body><rsp:ReceiveResponse>
                <rsp:Stream Name="stdout" CommandId="C1">{stdout_b64}</rsp:Stream>
                <rsp:CommandState CommandId="C1" State="http://schemas.microsoft.com/wbem/wsman/1/windows/shell/CommandState/Done">
                  <rsp:ExitCode>0</rsp:ExitCode>
                </rsp:CommandState>
              </rsp:ReceiveResponse></s:Body></s:Envelope>"#
        );
        let out = parse_receive(&xml).unwrap();
        assert_eq!(out.stdout, b"WIN-HOST\r\n");
        assert!(out.done);
        assert_eq!(out.exit_code, Some(0));
    }

    #[test]
    fn parses_receive_not_yet_done() {
        let xml = r#"<rsp:ReceiveResponse xmlns:rsp="y">
            <rsp:CommandState CommandId="C1" State="http://schemas.microsoft.com/wbem/wsman/1/windows/shell/CommandState/Running"/>
        </rsp:ReceiveResponse>"#;
        let out = parse_receive(xml).unwrap();
        assert!(!out.done);
        assert_eq!(out.exit_code, None);
    }
}
