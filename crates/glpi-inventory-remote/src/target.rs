// SPDX-License-Identifier: GPL-2.0-only

//! Remote target URLs (`ssh://`, `winrm://`).
//!
//! [`RemoteTarget`] parses the `remote` option values the agent accepts:
//! `scheme://[user[:password]@]host[:port][/path][?key=value&…]`. Only the
//! `ssh` and `winrm` schemes are recognised; everything else is rejected so a
//! typo never silently scans the wrong transport.

use std::collections::BTreeMap;

use glpi_core::error::{AgentError, Result};

/// The transport a [`RemoteTarget`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteScheme {
    /// SSH (Unix-like hosts).
    Ssh,
    /// WinRM (Windows hosts).
    WinRm,
}

impl RemoteScheme {
    /// Parses a scheme token (case-insensitive).
    fn parse(scheme: &str) -> Result<Self> {
        match scheme.to_ascii_lowercase().as_str() {
            "ssh" => Ok(Self::Ssh),
            "winrm" => Ok(Self::WinRm),
            other => Err(AgentError::Unsupported(format!(
                "remote scheme {other:?} (expected ssh or winrm)"
            ))),
        }
    }
}

/// A parsed remote-inventory target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTarget {
    /// Transport scheme.
    pub scheme: RemoteScheme,
    /// Login user, if given in the URL.
    pub user: Option<String>,
    /// Password, if given in the URL (URL-embedded credentials are discouraged
    /// but supported for parity with the Perl agent).
    pub password: Option<String>,
    /// Host name or IP literal.
    pub host: String,
    /// Explicit port, if given.
    pub port: Option<u16>,
    /// Extra `?key=value` options (e.g. `mode`, `assetname-support`).
    pub options: BTreeMap<String, String>,
}

impl RemoteTarget {
    /// Parses a `scheme://[user[:password]@]host[:port][/path][?query]` URL.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Unsupported`] for an unknown scheme and
    /// [`AgentError::Parse`] for a malformed URL (missing host or bad port).
    pub fn parse(url: &str) -> Result<Self> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| AgentError::Parse(format!("remote target {url:?} has no scheme")))?;
        let scheme = RemoteScheme::parse(scheme)?;

        // Split off the query string, then the path; only the authority matters.
        let (rest, query) = split_once_opt(rest, '?');
        let authority = rest.split('/').next().unwrap_or(rest);

        // Optional `user[:password]@` userinfo (split at the last '@' so a '@'
        // in the password does not confuse the host).
        let (userinfo, hostport) = match authority.rsplit_once('@') {
            Some((info, hp)) => (Some(info), hp),
            None => (None, authority),
        };
        let (user, password) = match userinfo {
            Some(info) => {
                let (u, p) = split_once_opt(info, ':');
                (non_empty(u), p.map(str::to_owned))
            }
            None => (None, None),
        };

        let (host, port) = parse_host_port(hostport)?;
        if host.is_empty() {
            return Err(AgentError::Parse(format!(
                "remote target {url:?} has no host"
            )));
        }

        let options = query.map(parse_query).unwrap_or_default();

        Ok(Self {
            scheme,
            user,
            password,
            host,
            port,
            options,
        })
    }

    /// Returns the value of a `?key=value` option, if present.
    #[must_use]
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(String::as_str)
    }
}

/// Splits `s` at the first `sep`, returning `(before, Some(after))` or
/// `(s, None)` when the separator is absent.
fn split_once_opt(s: &str, sep: char) -> (&str, Option<&str>) {
    match s.split_once(sep) {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    }
}

/// Maps an empty string to `None`.
fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

/// Parses `host[:port]`, handling `[ipv6]:port` bracket form.
fn parse_host_port(hostport: &str) -> Result<(String, Option<u16>)> {
    if let Some(rest) = hostport.strip_prefix('[') {
        // Bracketed IPv6 literal: `[::1]` or `[::1]:22`.
        let (host, after) = rest.split_once(']').ok_or_else(|| {
            AgentError::Parse(format!("unterminated IPv6 literal in {hostport:?}"))
        })?;
        let port = match after.strip_prefix(':') {
            Some(p) => Some(parse_port(p)?),
            None => None,
        };
        return Ok((host.to_owned(), port));
    }
    // More than one colon and no brackets: a bare IPv6 literal (a port would
    // need the `[addr]:port` form), so the whole string is the host.
    if hostport.matches(':').count() > 1 {
        return Ok((hostport.to_owned(), None));
    }
    match hostport.split_once(':') {
        Some((h, p)) => Ok((h.to_owned(), Some(parse_port(p)?))),
        None => Ok((hostport.to_owned(), None)),
    }
}

/// Parses a TCP port.
fn parse_port(port: &str) -> Result<u16> {
    port.parse()
        .map_err(|_| AgentError::Parse(format!("invalid port {port:?}")))
}

/// Parses a `key=value&key=value` query string.
fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = split_once_opt(pair, '=');
            (k.to_owned(), v.unwrap_or("").to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{RemoteScheme, RemoteTarget};

    #[test]
    fn parses_full_ssh_url() {
        let t = RemoteTarget::parse("ssh://admin:s3cret@10.0.0.5:2222/?mode=ssh").unwrap();
        assert_eq!(t.scheme, RemoteScheme::Ssh);
        assert_eq!(t.user.as_deref(), Some("admin"));
        assert_eq!(t.password.as_deref(), Some("s3cret"));
        assert_eq!(t.host, "10.0.0.5");
        assert_eq!(t.port, Some(2222));
        assert_eq!(t.option("mode"), Some("ssh"));
    }

    #[test]
    fn parses_bare_host() {
        let t = RemoteTarget::parse("ssh://server.example.com").unwrap();
        assert_eq!(t.host, "server.example.com");
        assert_eq!(t.user, None);
        assert_eq!(t.port, None);
    }

    #[test]
    fn parses_winrm_and_ipv6() {
        let t = RemoteTarget::parse("winrm://user@[2001:db8::1]:5986").unwrap();
        assert_eq!(t.scheme, RemoteScheme::WinRm);
        assert_eq!(t.host, "2001:db8::1");
        assert_eq!(t.port, Some(5986));
        assert_eq!(t.user.as_deref(), Some("user"));
    }

    #[test]
    fn rejects_unknown_scheme_and_missing_parts() {
        assert!(RemoteTarget::parse("telnet://host").is_err());
        assert!(RemoteTarget::parse("host-without-scheme").is_err());
        assert!(RemoteTarget::parse("ssh://host:notaport").is_err());
    }

    #[test]
    fn keeps_password_with_at_sign() {
        let t = RemoteTarget::parse("ssh://u:p@ss@host").unwrap();
        assert_eq!(t.user.as_deref(), Some("u"));
        assert_eq!(t.password.as_deref(), Some("p@ss"));
        assert_eq!(t.host, "host");
    }
}
