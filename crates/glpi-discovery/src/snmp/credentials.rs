// SPDX-License-Identifier: GPL-2.0-only

//! Translation from the agent's [`SnmpCredentials`] into `snmp2`'s session
//! parameters.
//!
//! The agent models credentials protocol-agnostically (`glpi-core`); `snmp2`
//! wants a community string for v1/v2c and a [`Security`] value for v3. This
//! module maps between them, deriving the v3 security level from which fields
//! are present and selecting the correct AES key-localization method:
//!
//! * standard AES-192/256 → [`KeyExtension::Blumenthal`];
//! * the Cisco "AES-192-C / AES-256-C" variants → [`KeyExtension::Reeder`].
//!
//! The mapping helpers are pure and unit-tested; assembling the opaque
//! [`Security`] is validated only for success/failure (its fields are private).

use glpi_core::error::{AgentError, Result};
use glpi_core::types::snmp::{AuthProtocol, PrivProtocol, SnmpCredentials, SnmpVersion};
use snmp2::v3::{Auth, AuthProtocol as V3Auth, Cipher, KeyExtension, Security};

/// SNMPv3 User-based Security Model security level, derived from the credential
/// fields that are populated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// No authentication, no privacy.
    NoAuthNoPriv,
    /// Authentication only.
    AuthNoPriv,
    /// Authentication and privacy (encryption).
    AuthPriv,
}

/// Determines the v3 security level implied by `creds`.
///
/// Authentication needs both an algorithm and a passphrase; privacy needs both
/// plus authentication underneath it.
///
/// # Errors
///
/// Returns [`AgentError::Config`] if privacy is requested without
/// authentication (an invalid USM combination).
pub fn security_level(creds: &SnmpCredentials) -> Result<SecurityLevel> {
    let has_auth = creds.auth_protocol.is_some() && creds.auth_password.is_some();
    let has_priv = creds.priv_protocol.is_some() && creds.priv_password.is_some();
    match (has_auth, has_priv) {
        (false, false) => Ok(SecurityLevel::NoAuthNoPriv),
        (true, false) => Ok(SecurityLevel::AuthNoPriv),
        (true, true) => Ok(SecurityLevel::AuthPriv),
        (false, true) => Err(AgentError::Config(
            "SNMPv3 privacy requires authentication".to_owned(),
        )),
    }
}

/// Maps an agent auth algorithm onto the `snmp2` variant.
#[must_use]
pub fn map_auth_protocol(protocol: AuthProtocol) -> V3Auth {
    match protocol {
        AuthProtocol::Md5 => V3Auth::Md5,
        AuthProtocol::Sha1 => V3Auth::Sha1,
        AuthProtocol::Sha224 => V3Auth::Sha224,
        AuthProtocol::Sha256 => V3Auth::Sha256,
        AuthProtocol::Sha384 => V3Auth::Sha384,
        AuthProtocol::Sha512 => V3Auth::Sha512,
    }
}

/// Maps an agent privacy algorithm onto the `snmp2` cipher (dropping the
/// Cisco-vs-standard distinction, which is carried by the key extension).
#[must_use]
pub fn map_priv_cipher(protocol: PrivProtocol) -> Cipher {
    match protocol {
        PrivProtocol::Des => Cipher::Des,
        PrivProtocol::Aes128 => Cipher::Aes128,
        PrivProtocol::Aes192 | PrivProtocol::Aes192c => Cipher::Aes192,
        PrivProtocol::Aes256 | PrivProtocol::Aes256c => Cipher::Aes256,
    }
}

/// Selects the AES key-localization method for a privacy algorithm.
///
/// The Cisco "C" variants use Reeder's extension; everything else uses
/// Blumenthal's. (Only relevant for AES-192/256; harmless otherwise.)
#[must_use]
pub fn priv_key_extension(protocol: PrivProtocol) -> KeyExtension {
    match protocol {
        PrivProtocol::Aes192c | PrivProtocol::Aes256c => KeyExtension::Reeder,
        _ => KeyExtension::Blumenthal,
    }
}

/// Returns the community string for a v1/v2c credential set.
///
/// # Errors
///
/// Returns [`AgentError::Config`] if the credentials are for v3, or if no
/// community string is set.
pub fn community(creds: &SnmpCredentials) -> Result<&[u8]> {
    if creds.version == SnmpVersion::V3 {
        return Err(AgentError::Config(
            "v3 credentials have no community string".to_owned(),
        ));
    }
    creds
        .community
        .as_deref()
        .map(str::as_bytes)
        .ok_or_else(|| AgentError::Config("SNMP v1/v2c requires a community string".to_owned()))
}

/// Builds an `snmp2` [`Security`] from a v3 credential set.
///
/// # Errors
///
/// Returns [`AgentError::Config`] if the username is missing, a required
/// passphrase is absent, or the auth/priv combination is invalid.
pub fn build_security(creds: &SnmpCredentials) -> Result<Security> {
    let username = creds
        .username
        .as_deref()
        .filter(|u| !u.is_empty())
        .ok_or_else(|| AgentError::Config("SNMPv3 requires a username".to_owned()))?;

    let level = security_level(creds)?;
    let auth_password = creds.auth_password.as_deref().unwrap_or_default();

    let security = match level {
        SecurityLevel::NoAuthNoPriv => {
            Security::new(username.as_bytes(), b"").with_auth(Auth::NoAuthNoPriv)
        }
        SecurityLevel::AuthNoPriv => {
            let protocol = creds.auth_protocol.expect("auth level implies a protocol");
            Security::new(username.as_bytes(), auth_password.as_bytes())
                .with_auth(Auth::AuthNoPriv)
                .with_auth_protocol(map_auth_protocol(protocol))
        }
        SecurityLevel::AuthPriv => {
            let auth = creds.auth_protocol.expect("auth level implies a protocol");
            let privacy = creds.priv_protocol.expect("priv level implies a protocol");
            let privacy_password = creds
                .priv_password
                .as_deref()
                .expect("priv level implies a passphrase")
                .as_bytes()
                .to_vec();
            Security::new(username.as_bytes(), auth_password.as_bytes())
                .with_auth(Auth::AuthPriv {
                    cipher: map_priv_cipher(privacy),
                    privacy_password,
                })
                .with_auth_protocol(map_auth_protocol(auth))
                .with_key_extension_method(priv_key_extension(privacy))
        }
    };

    // snmp2 0.5 cannot transmit a non-default contextName (it only parses one),
    // so surface it rather than silently misleading the operator.
    if creds.context_name.as_deref().is_some_and(|c| !c.is_empty()) {
        tracing::warn!(
            "SNMPv3 contextName is configured but snmp2 0.5 cannot send a non-default context; \
             it will be ignored"
        );
    }

    Ok(security)
}

#[cfg(test)]
mod tests {
    use super::{
        build_security, community, map_auth_protocol, map_priv_cipher, priv_key_extension,
        security_level, SecurityLevel,
    };
    use glpi_core::types::snmp::{AuthProtocol, PrivProtocol, SnmpCredentials, SnmpVersion};
    use snmp2::v3::{AuthProtocol as V3Auth, Cipher, KeyExtension};

    fn v3() -> SnmpCredentials {
        SnmpCredentials {
            version: SnmpVersion::V3,
            username: Some("monitor".to_owned()),
            ..SnmpCredentials::default()
        }
    }

    #[test]
    fn security_level_follows_populated_fields() {
        let mut creds = v3();
        assert_eq!(security_level(&creds).unwrap(), SecurityLevel::NoAuthNoPriv);

        creds.auth_protocol = Some(AuthProtocol::Sha256);
        creds.auth_password = Some("authpass".to_owned());
        assert_eq!(security_level(&creds).unwrap(), SecurityLevel::AuthNoPriv);

        creds.priv_protocol = Some(PrivProtocol::Aes128);
        creds.priv_password = Some("privpass".to_owned());
        assert_eq!(security_level(&creds).unwrap(), SecurityLevel::AuthPriv);
    }

    #[test]
    fn privacy_without_authentication_is_rejected() {
        let mut creds = v3();
        creds.priv_protocol = Some(PrivProtocol::Aes128);
        creds.priv_password = Some("privpass".to_owned());
        assert!(security_level(&creds).is_err());
    }

    #[test]
    fn auth_protocol_mapping_is_exhaustive() {
        assert_eq!(map_auth_protocol(AuthProtocol::Md5), V3Auth::Md5);
        assert_eq!(map_auth_protocol(AuthProtocol::Sha1), V3Auth::Sha1);
        assert_eq!(map_auth_protocol(AuthProtocol::Sha224), V3Auth::Sha224);
        assert_eq!(map_auth_protocol(AuthProtocol::Sha256), V3Auth::Sha256);
        assert_eq!(map_auth_protocol(AuthProtocol::Sha384), V3Auth::Sha384);
        assert_eq!(map_auth_protocol(AuthProtocol::Sha512), V3Auth::Sha512);
    }

    #[test]
    fn cisco_variants_select_reeder_extension() {
        // Standard AES uses Blumenthal; the Cisco "C" variants use Reeder, but
        // both map onto the same underlying cipher width.
        assert_eq!(map_priv_cipher(PrivProtocol::Aes192), Cipher::Aes192);
        assert_eq!(map_priv_cipher(PrivProtocol::Aes192c), Cipher::Aes192);
        assert_eq!(
            priv_key_extension(PrivProtocol::Aes192),
            KeyExtension::Blumenthal
        );
        assert_eq!(
            priv_key_extension(PrivProtocol::Aes192c),
            KeyExtension::Reeder
        );
        assert_eq!(map_priv_cipher(PrivProtocol::Aes256c), Cipher::Aes256);
        assert_eq!(
            priv_key_extension(PrivProtocol::Aes256c),
            KeyExtension::Reeder
        );
    }

    #[test]
    fn community_requires_v1v2c_with_a_string() {
        assert!(community(&v3()).is_err());
        assert_eq!(
            community(&SnmpCredentials::v2c("public")).unwrap(),
            b"public"
        );

        let empty = SnmpCredentials {
            version: SnmpVersion::V2c,
            ..SnmpCredentials::default()
        };
        assert!(community(&empty).is_err());
    }

    #[test]
    fn build_security_validates_username_and_accepts_full_authpriv() {
        // Missing username.
        let mut creds = SnmpCredentials {
            version: SnmpVersion::V3,
            ..SnmpCredentials::default()
        };
        assert!(build_security(&creds).is_err());

        // Complete authPriv credentials build successfully.
        creds.username = Some("monitor".to_owned());
        creds.auth_protocol = Some(AuthProtocol::Sha512);
        creds.auth_password = Some("authpass".to_owned());
        creds.priv_protocol = Some(PrivProtocol::Aes256c);
        creds.priv_password = Some("privpass".to_owned());
        assert!(build_security(&creds).is_ok());
    }
}
