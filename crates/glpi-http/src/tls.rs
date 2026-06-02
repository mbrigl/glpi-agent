// SPDX-License-Identifier: GPL-2.0-only

//! HTTPS listener for the SSL server plugin.
//!
//! Ported from `GLPI::Agent::HTTP::Server::SSL`: serves the control router over
//! TLS on the plugin's dedicated port using the configured PEM certificate and
//! key. [`server_config`] turns a validated
//! [`SslConfig`](glpi_plugins::ssl::SslConfig) into a rustls
//! [`ServerConfig`](rustls::ServerConfig); [`serve_tls`] runs the accept loop,
//! terminating TLS and feeding each connection to the axum [`Router`], with the
//! peer address injected so the trust middleware still applies.

use std::sync::Arc;

use axum::extract::ConnectInfo;
use axum::Router;
use base64::Engine;
use glpi_core::error::{AgentError, Result};
use glpi_plugins::ssl::SslConfig;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use rustls::pki_types::{
    CertificateDer, PrivateKeyDer, PrivatePkcs1KeyDer, PrivatePkcs8KeyDer, PrivateSec1KeyDer,
};
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower_service::Service;

/// Builds a rustls [`ServerConfig`] from the SSL plugin's certificate and key.
///
/// The plugin's `ssl_cert_file` / `ssl_key_file` are read as PEM; the key may
/// be PKCS#8 (`PRIVATE KEY`), PKCS#1 (`RSA PRIVATE KEY`) or SEC1
/// (`EC PRIVATE KEY`).
///
/// # Errors
///
/// [`AgentError::Config`] if a path is unset, a file cannot be read, or the
/// PEM contains no certificate / no usable private key, or rustls rejects the
/// pair.
pub fn server_config(ssl: &SslConfig) -> Result<ServerConfig> {
    // rustls 0.23 needs a process-wide crypto provider; install the default
    // (aws-lc-rs) once. An `Err` means another component already installed one.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let cert_path = ssl
        .ssl_cert_file
        .as_deref()
        .ok_or_else(|| AgentError::Config("SSL plugin: no ssl_cert_file".to_owned()))?;
    let key_path = ssl
        .ssl_key_file
        .as_deref()
        .ok_or_else(|| AgentError::Config("SSL plugin: no ssl_key_file".to_owned()))?;

    let cert_pem = std::fs::read_to_string(cert_path)
        .map_err(|e| AgentError::Config(format!("cannot read ssl_cert_file {cert_path}: {e}")))?;
    let key_pem = std::fs::read_to_string(key_path)
        .map_err(|e| AgentError::Config(format!("cannot read ssl_key_file {key_path}: {e}")))?;

    let certs: Vec<CertificateDer<'static>> = pem_blocks(&cert_pem, "CERTIFICATE")
        .into_iter()
        .map(CertificateDer::from)
        .collect();
    if certs.is_empty() {
        return Err(AgentError::Config(format!(
            "no CERTIFICATE block in ssl_cert_file {cert_path}"
        )));
    }
    let key = private_key(&key_pem)
        .ok_or_else(|| AgentError::Config(format!("no private key in ssl_key_file {key_path}")))?;

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| AgentError::Config(format!("invalid SSL certificate/key pair: {e}")))
}

/// Serves `router` over TLS on an already-bound `listener` until the process
/// ends. Each accepted connection is TLS-terminated and the peer address is
/// inserted as [`ConnectInfo`] so the trust middleware behaves as on plain HTTP.
///
/// # Errors
///
/// Returns an error only if the accept loop fails irrecoverably; per-connection
/// handshake/serve failures are logged and dropped.
pub async fn serve_tls(listener: TcpListener, router: Router, config: ServerConfig) -> Result<()> {
    let acceptor = TlsAcceptor::from(Arc::new(config));
    loop {
        let (tcp, remote) = listener
            .accept()
            .await
            .map_err(|e| AgentError::Transport(format!("HTTPS accept failed: {e}")))?;
        let acceptor = acceptor.clone();
        let router = router.clone();
        tokio::spawn(async move {
            let tls = match acceptor.accept(tcp).await {
                Ok(tls) => tls,
                Err(e) => {
                    tracing::debug!(client = %remote, error = %e, "TLS handshake failed");
                    return;
                }
            };
            let io = TokioIo::new(tls);
            let service =
                hyper::service::service_fn(move |mut request: hyper::Request<Incoming>| {
                    request.extensions_mut().insert(ConnectInfo(remote));
                    router.clone().call(request)
                });
            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                tracing::debug!(client = %remote, error = %e, "HTTPS connection error");
            }
        });
    }
}

/// Extracts the DER bodies of every `-----BEGIN {tag}-----` block in `pem`.
fn pem_blocks(pem: &str, tag: &str) -> Vec<Vec<u8>> {
    let begin = format!("-----BEGIN {tag}-----");
    let end = format!("-----END {tag}-----");
    let mut blocks = Vec::new();
    let mut rest = pem;
    while let Some(start) = rest.find(&begin) {
        let after = &rest[start + begin.len()..];
        let Some(stop) = after.find(&end) else { break };
        let b64: String = after[..stop].split_whitespace().collect();
        if let Ok(der) = base64::engine::general_purpose::STANDARD.decode(b64.as_bytes()) {
            blocks.push(der);
        }
        rest = &after[stop + end.len()..];
    }
    blocks
}

/// Reads the first private key from `pem`, trying PKCS#8, PKCS#1 then SEC1.
fn private_key(pem: &str) -> Option<PrivateKeyDer<'static>> {
    if let Some(der) = pem_blocks(pem, "PRIVATE KEY").into_iter().next() {
        return Some(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(der)));
    }
    if let Some(der) = pem_blocks(pem, "RSA PRIVATE KEY").into_iter().next() {
        return Some(PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(der)));
    }
    if let Some(der) = pem_blocks(pem, "EC PRIVATE KEY").into_iter().next() {
        return Some(PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(der)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{serve_tls, server_config};
    use crate::trust::TrustList;
    use crate::HttpServer;
    use glpi_plugins::ssl::SslConfig;

    // A self-signed localhost certificate (CN/SAN=127.0.0.1, ~100y validity)
    // used only to exercise the TLS path in tests.
    const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----
MIIDJzCCAg+gAwIBAgIUIInSkoVtzzPX31csPXSEib7cEWkwDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MDYwMjEzMjIzNFoYDzIxMjYw
NTA5MTMyMjM0WjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQCcx3ydWg8Ky3No82NYjpeIma2YLw5BMtAKw1K4A1iN
AEMEiKlF+kZFzFF58NtyUWl5Q8Q99bECvE17NGScCZjp8vhszBJig3HQhJCUda8i
2hhpKXm+s42/BtDe9wUX7WVj+SEYrItSf6NNw7pbw7aQHPJ3pblYJuklKv5cB/84
k6HVispZa6lqldve9UXFtQ+7f2X5+VOcqxKYV+iknrFicD2NwVXGDYT+vpnURdPc
Mw14bRCX4YbsBSkBc6a/85tIq9DTV8PfW/1Mu090aPCpMr8jytL6mvTRIYWDqaho
RPdOpLjulvXlaKDW1QAO/CHmhAIJk0zfgB/VFwYxZa+jAgMBAAGjbzBtMB0GA1Ud
DgQWBBQptbEifOkxIuh6mnAF1tCDw39e8jAfBgNVHSMEGDAWgBQptbEifOkxIuh6
mnAF1tCDw39e8jAPBgNVHRMBAf8EBTADAQH/MBoGA1UdEQQTMBGHBH8AAAGCCWxv
Y2FsaG9zdDANBgkqhkiG9w0BAQsFAAOCAQEABRghsPwk8XHE+HQb9Qm1ewtDaNlM
LiTgLZkWXFMaaKKwVK3Pb56MUMuO+0N0k+XZasLip/ByAfIsQkK4Um9Hamk2cWgF
+/AImsnVkeZwgbbClLSCDqtQNYlEHT09hZKokimaQrIHqzCSRoL3lGpQ2vSV/1ww
2qaibDhOEle+WBE7dZfbLYrzqZ31kcOmwNfbbCAdfHjdyADdRCknS2t/W8Nyx5Ij
2feQfATV0+1OIpfpOeVzw458f5K+FmCZB7WSxHu6rP8i1ur3GYKnAqmGefSR7Jeb
J4V+hzuqR+JeS5TrWtmV5z/qsmmcR4TIy4iFkdy3g/ZXTA5KgxCeCPm+gw==
-----END CERTIFICATE-----
";

    const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCcx3ydWg8Ky3No
82NYjpeIma2YLw5BMtAKw1K4A1iNAEMEiKlF+kZFzFF58NtyUWl5Q8Q99bECvE17
NGScCZjp8vhszBJig3HQhJCUda8i2hhpKXm+s42/BtDe9wUX7WVj+SEYrItSf6NN
w7pbw7aQHPJ3pblYJuklKv5cB/84k6HVispZa6lqldve9UXFtQ+7f2X5+VOcqxKY
V+iknrFicD2NwVXGDYT+vpnURdPcMw14bRCX4YbsBSkBc6a/85tIq9DTV8PfW/1M
u090aPCpMr8jytL6mvTRIYWDqahoRPdOpLjulvXlaKDW1QAO/CHmhAIJk0zfgB/V
FwYxZa+jAgMBAAECggEAKRpr410fIHdilJtq6mbH97pCtulvVUybGpdG8pN9/cmZ
yHCD4KLTFa2RluS8w+XwPyizJINrmwn/TlPYJMinXH6k/vEpyMYpar+2oBWSixKe
38NN9d9hRDnnPO2KWlGVCXbhZHSoOkLYb6TnEPPowzOzpga+5wuciHATK9G06gHx
jPhHO+pp74ACF40QPzLf/M9AY0r+m3OyY/SfaZbh8/XcUdIFS8DEbYRRzch5l9L6
k6SR6zt/P1nrK5sX+Zome/ZXwPkGY1QCgRC+kC7MY4AnBEcNWDl9B2mmSSIUmtEv
fPdcbs27n12tMeZ1koxaxV1x6G9TaFOOxOK2s3VMYQKBgQDOkxkK0f+yyyQIzUnV
HvN7bxCgaWIRt5wf0DI5q2HImlAjG7sbnZBBwGm7yMUXQ4iv0viMd+OiZGa4pFdm
ZPe6QfA72MSTbXLrsIakhP+UCtim/EQ3c6/80CGBbA7z8Ua8W4/jbCRingyuyDTu
taSM8HpuEGFKZNh0UNbQ5KCEiwKBgQDCSl+RZdgLkpKMqVPVyLW7R6V+dQlMWPDT
BYwNGrKIA1aKjXa66+DB8Wj91qpvLNDUcbUn5rKdPSJ/qFFyvAlB3sKtoemVwUm8
767RXJLNnEmq5/5+N0iTzrg8T4BHB+gsu7uQ+GNXhCEGeboHvSOIqI7J6SUy2PQH
DSdF3YgsSQKBgQCnWpiBIZxL1zM8RkQ4eri7GUGZE4c131CGnX7zJZs0j3+40bCG
MOI7woxma8Lwk+/ascpW0pICb+CgWdPMyqO/q8faVET9Q0BFHWAXTQBZiWf38Iu7
eOfsoxlh5o8+pguucWdi0auwkWao+t2XPmUvIWuuW2rWgFiz8wH1fiNk/QKBgQC5
NP5+8q+M8I7kqXEyRJ8ARN78egKAFfSTpCEKSN3RDCWN9CYvLzVUi5UDDIPxcK4t
JauDusWfYCyntkLV9Wt5sCiyLbsmN1fcVDq4dt+2QnpzAa22kWqNA6zaSQrGK0Jm
ihrVqgHA5kI5EwaD5AegeNWMocQFAY01v5MlZXUuiQKBgC0W8UjaUOUio17H3+P6
qJZJJKhgEr7xXuUIpip//c0QScxw7iQYKDEKDfGCpgpEdL4UVrMyf3S4kh37Jfn4
2aQxKcVCNVGQ2S/OBLq1qCBlvdSodG1TbifTXNFASnpwnM+GXFLUEoT58pu/XTNx
z1UsNoLEK+VP99fkrs+wlI2w
-----END PRIVATE KEY-----
";

    fn fixture_ssl(dir: &std::path::Path) -> SslConfig {
        let cert = dir.join("cert.pem");
        let key = dir.join("key.pem");
        std::fs::write(&cert, TEST_CERT).unwrap();
        std::fs::write(&key, TEST_KEY).unwrap();
        SslConfig {
            disabled: false,
            port: 0,
            ssl_cert_file: Some(cert.to_str().unwrap().to_owned()),
            ssl_key_file: Some(key.to_str().unwrap().to_owned()),
            ssl_cipher: None,
            forbid_not_trusted: false,
        }
    }

    #[test]
    fn server_config_requires_a_cert_path() {
        let ssl = SslConfig {
            disabled: false,
            ..SslConfig::default()
        };
        assert!(server_config(&ssl).is_err());
    }

    #[test]
    fn builds_a_server_config_from_pem() {
        let dir = tempfile::tempdir().unwrap();
        assert!(server_config(&fixture_ssl(dir.path())).is_ok());
    }

    #[tokio::test]
    async fn serves_https_to_a_trusted_client() {
        let dir = tempfile::tempdir().unwrap();
        let config = server_config(&fixture_ssl(dir.path())).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (server, _rx) = HttpServer::new(
            "127.0.0.1".parse().unwrap(),
            0,
            TrustList::default(),
            "tls test",
        );
        let router = server.router();
        tokio::spawn(serve_tls(listener, router, config));

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let response = client
            .get(format!("https://127.0.0.1:{port}/status"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(response.text().await.unwrap().contains("tls test"));
    }
}
