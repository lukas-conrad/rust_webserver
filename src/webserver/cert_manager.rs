use crate::config::CertificateConfig;
use crate::file_watcher::FileWatcher;
use log::{error, info};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::server::ResolvesServerCert;
use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

/// Custom certificate resolver with wildcard domain matching support
#[derive(Debug)]
pub struct WildcardCertResolver {
    /// Maps exact domain names to their certificates
    certs: HashMap<String, Arc<CertifiedKey>>,
}

impl WildcardCertResolver {
    pub fn new() -> Self {
        Self {
            certs: HashMap::new(),
        }
    }

    /// Add one or more domain certificates (supports wildcard domains like *.example.com)
    pub fn add(&mut self, domains: &[String], cert: CertifiedKey) -> Result<(), String> {
        let arc_cert = Arc::new(cert);

        for domain in domains {
            let domain = domain.trim().to_string();
            self.certs.insert(domain, arc_cert.clone());
        }

        Ok(())
    }

    /// Check if a domain matches a wildcard base (e.g., "api.example.com" matches "example.com")
    pub fn matches_wildcard(domain: &str, base: &str) -> bool {
        let domain = domain.to_lowercase();
        let base = base.to_lowercase();

        if domain == base {
            return true;
        }

        if let Some((prefix, suffix)) = domain.split_once('.') {
            if suffix == base && !prefix.contains('.') {
                return true;
            }
        }

        false
    }
}

impl ResolvesServerCert for WildcardCertResolver {
    fn resolve(
        &self,
        client_hello: tokio_rustls::rustls::server::ClientHello,
    ) -> Option<Arc<CertifiedKey>> {
        let server_name_str = match client_hello.server_name() {
            Some(name) => name.to_lowercase(),
            None => {
                info!("No SNI provided, rejecting connection.");
                return None;
            }
        };

        // 1. Try exact match first
        if let Some(cert) = self.certs.get(&server_name_str) {
            info!(
                "Certificate found for exact domain match: {}",
                server_name_str
            );
            return Some(cert.clone());
        }

        // 2. Try wildcard matching
        if let Some((_, domain)) = server_name_str.split_once('.') {
            let wildcard_name = format!("*.{}", domain);

            if let Some(cert) = self.certs.get(&wildcard_name) {
                info!(
                    "Certificate found for wildcard domain match: {}",
                    server_name_str
                );
                return Some(cert.clone());
            }
        }

        error!("No certificate found for domain: {}", server_name_str);

        None
    }
}

pub struct CertificateManager {}

impl CertificateManager {
    /// Build a single ServerConfig handling multiple certificates via SNI
    pub async fn create_updating_acceptor(
        certificates: &[CertificateConfig],
    ) -> Result<Arc<RwLock<TlsAcceptor>>, Box<dyn Error + Send + Sync>> {
        let config = Self::create_sni_resolver(certificates)?;

        let acceptor = Arc::new(RwLock::new(TlsAcceptor::from(Arc::new(config))));

        let paths = certificates
            .iter()
            .map(|cfg| vec![cfg.cert_path.clone(), cfg.key_path.clone()])
            .flatten()
            .map(|path| PathBuf::from(path))
            .collect();

        let certificates = certificates.to_vec();
        let mut watcher = cloned!(acceptor, certificates; FileWatcher::new(paths, Arc::new(move |_| {

            match Self::create_sni_resolver(certificates.as_slice()) {
                Ok(config) => {
                    spawn_cloned!(acceptor, config; async move  {
                        *acceptor.write().await = TlsAcceptor::from(Arc::new(config));
                    });
                },
                Err(e) => {error!("Error when creating sni resolver: {}", e)}
            };
        })))?;
        watcher.start().await?;

        Ok(acceptor)
    }

    fn create_sni_resolver(
        certificates: &[CertificateConfig],
    ) -> Result<ServerConfig, Box<dyn Error + Send + Sync>> {
        let mut resolver = WildcardCertResolver::new();

        let provider = tokio_rustls::rustls::crypto::ring::default_provider();

        for cert_config in certificates {
            let certs = load_certs(&cert_config.cert_path)?;

            let extracted_domains = Self::extract_domains(&certs);

            if extracted_domains.is_empty() {
                error!(
                    "No valid domains found in certificate {:?}, skipping",
                    cert_config.cert_path
                );
                continue;
            }

            info!(
                "Certificate ({:?}) covers these domains: {:?}",
                cert_config.cert_path, extracted_domains
            );

            let key = load_key(&cert_config.key_path)?;

            let signing_key = provider.key_provider.load_private_key(key).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Invalid key for certificate {:?}: {:?}",
                        cert_config.cert_path, e
                    ),
                )
            })?;

            let certified_key = CertifiedKey::new(certs, signing_key);

            resolver
                .add(&extracted_domains, certified_key)
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("SNI error for {:?}: {e}", extracted_domains),
                    )
                })?;

            info!("Added SNI certificate mapping for {:?}", extracted_domains);
        }

        let config = ServerConfig::builder_with_provider(Arc::new(provider.clone()))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));
        Ok(config)
    }

    fn extract_domains(certs: &Vec<CertificateDer>) -> Vec<String> {
        let mut extracted_domains: Vec<String> = vec![];
        for cert_der in certs {
            match x509_parser::parse_x509_certificate(cert_der.as_ref()) {
                Ok((_, cert)) => {
                    let mut domains_in_cert = Vec::new();

                    if let Ok(Some(san_ext)) = cert.subject_alternative_name() {
                        for name in san_ext.value.general_names.iter() {
                            if let x509_parser::prelude::GeneralName::DNSName(dns) = name {
                                domains_in_cert.push(dns.to_string());
                            }
                        }
                    }

                    if domains_in_cert.is_empty() {
                        if let Some(cn) = cert.subject().iter_common_name().next() {
                            if let Ok(cn_str) = cn.attr_value().as_str() {
                                domains_in_cert.push(cn_str.to_string());
                            }
                        }
                    }

                    extracted_domains.extend(domains_in_cert);
                }
                Err(e) => {
                    error!("Error parsing certificate to read SAN: {:?}", e);
                }
            }
        }

        extracted_domains.sort();
        extracted_domains.dedup();

        extracted_domains
    }
}

/// Load certificates from PEM file
fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, std::io::Error> {
    let certfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(certfile);
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .filter_map(|res| res.ok())
        .collect();
    if certs.is_empty() {
        error!("No certificates found in {path}");
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No certificates found",
        ));
    } else {
        info!("{} certificates loaded from {path}", certs.len());
    }
    Ok(certs)
}

/// Load private key from PEM file (supports PKCS#8, RSA, and EC formats)
fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, std::io::Error> {
    let keyfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(keyfile);

    // Try PKCS#8 first
    {
        let mut pkcs8_keys =
            rustls_pemfile::pkcs8_private_keys(&mut reader).filter_map(|res| res.ok());
        if let Some(key) = pkcs8_keys.next() {
            info!("PKCS#8 private key loaded from {path}");
            return Ok(PrivateKeyDer::Pkcs8(key));
        }
    }

    // Seek back to beginning and try RSA
    reader.seek(SeekFrom::Start(0)).map_err(|e| {
        error!("Failed to seek in key file {path}: {e}");
        e
    })?;
    {
        let mut rsa_keys = rustls_pemfile::rsa_private_keys(&mut reader).filter_map(|res| res.ok());
        if let Some(key) = rsa_keys.next() {
            info!("RSA private key loaded from {path}");
            return Ok(PrivateKeyDer::Pkcs1(key));
        }
    }

    // Seek back to beginning and try EC
    reader.seek(SeekFrom::Start(0)).map_err(|e| {
        error!("Failed to seek in key file {path}: {e}");
        e
    })?;
    {
        let mut ec_keys = rustls_pemfile::ec_private_keys(&mut reader).filter_map(|res| res.ok());
        if let Some(key) = ec_keys.next() {
            info!("EC private key loaded from {path}");
            return Ok(PrivateKeyDer::Sec1(key));
        }
    }

    error!("No private key found in {path} (neither PKCS#8, RSA, nor EC)");
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "No private key found (neither PKCS#8, RSA, nor EC)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::CertificateParams;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::NamedTempFile;
    use tokio_rustls::rustls::{ClientConfig, ClientConnection, ServerConfig, ServerConnection};

    /// Helper to generate a self-signed cert for given domains and save cert + key to temp files.
    fn store_self_signed_cert(domains: Vec<String>) -> (NamedTempFile, NamedTempFile) {
        let params = CertificateParams::new(domains.clone());
        let cert = rcgen::Certificate::from_params(params).unwrap();
        let cert_pem = cert.serialize_pem().unwrap();
        let key_pem = cert.serialize_private_key_pem();

        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(cert_pem.as_bytes()).unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(key_pem.as_bytes()).unwrap();

        (cert_file, key_file)
    }

    #[derive(Debug)]
    struct NoCertVerifier;

    impl tokio_rustls::rustls::client::danger::ServerCertVerifier for NoCertVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
            _server_name: &tokio_rustls::rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: tokio_rustls::rustls::pki_types::UnixTime,
        ) -> Result<tokio_rustls::rustls::client::danger::ServerCertVerified, tokio_rustls::rustls::Error> {
            Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
            _dss: &tokio_rustls::rustls::DigitallySignedStruct,
        ) -> Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
            _dss: &tokio_rustls::rustls::DigitallySignedStruct,
        ) -> Result<tokio_rustls::rustls::client::danger::HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
            vec![
                tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA256,
                tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                tokio_rustls::rustls::SignatureScheme::ED25519,
            ]
        }
    }

    /// Helper to test in-memory TLS handshake
    fn check_ssl_handshake(server_config: Arc<ServerConfig>, domain: &str) -> Result<(), Box<dyn Error>> {
        let server_name = rustls::pki_types::ServerName::try_from(domain)?.to_owned();

        let mut client_config = ClientConfig::builder()
            .with_root_certificates(tokio_rustls::rustls::RootCertStore::empty())
            .with_no_client_auth();

        client_config.dangerous().set_certificate_verifier(Arc::new(NoCertVerifier));

        let mut client = ClientConnection::new(Arc::new(client_config), server_name).unwrap();
        let mut server = ServerConnection::new(server_config).unwrap();

        let mut client_buf = Vec::new();
        let mut server_buf = Vec::new();

        loop {
            if client.wants_write() {
                client.write_tls(&mut client_buf).unwrap();
            }

            if !client_buf.is_empty() {
                let mut cursor = std::io::Cursor::new(&client_buf);
                server.read_tls(&mut cursor)?;
                server.process_new_packets()?;
                client_buf.clear();
            }

            if server.wants_write() {
                server.write_tls(&mut server_buf).unwrap();
            }

            if !server_buf.is_empty() {
                let mut cursor = std::io::Cursor::new(&server_buf);
                client.read_tls(&mut cursor)?;
                client.process_new_packets()?;
                server_buf.clear();
            }

            if !client.is_handshaking() && !server.is_handshaking() {
                break;
            }
        }

        Ok(())
    }

    #[test]
    fn test_exact_domain_match() {
        let (cert_file, key_file) = store_self_signed_cert(vec!["example.com".to_string()]);

        let cert_config = CertificateConfig {
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
        };

        let server_config = CertificateManager::create_sni_resolver(&[cert_config]).unwrap();

        // Exact match should succeed
        assert!(check_ssl_handshake(Arc::new(server_config), "example.com").is_ok());
    }

    #[test]
    fn test_wildcard_domain_match() {
        let (cert_file, key_file) = store_self_signed_cert(vec!["*.example.com".to_string()]);

        let cert_config = CertificateConfig {
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
        };

        let server_config = CertificateManager::create_sni_resolver(&[cert_config]).unwrap();

        // Match subdomain against wildcard should succeed
        assert!(check_ssl_handshake(Arc::new(server_config.clone()), "sub.example.com").is_ok());
        assert!(check_ssl_handshake(Arc::new(server_config), "another.example.com").is_ok());
    }

    #[test]
    fn test_unknown_domain_fails() {
        let (cert_file, key_file) = store_self_signed_cert(vec!["example.com".to_string()]);

        let cert_config = CertificateConfig {
            cert_path: cert_file.path().to_str().unwrap().to_string(),
            key_path: key_file.path().to_str().unwrap().to_string(),
        };

        let server_config = CertificateManager::create_sni_resolver(&[cert_config]).unwrap();

        // Unknown domain should fail handshake (Server throws internal error because no cert found)
        let res = check_ssl_handshake(Arc::new(server_config), "unknown.com");
        assert!(res.is_err());
    }
}
