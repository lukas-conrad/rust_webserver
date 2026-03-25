use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Error as IoError, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{error, info};

use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::{ServerConfig};
use tokio_rustls::rustls::server::ResolvesServerCertUsingSni;
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_rustls::rustls::sign::CertifiedKey;
use crate::config::DomainConfig;

/// Manages TLS certificates for multiple domains with caching
pub struct CertificateManager {
    /// Cache of loaded TLS server configurations keyed by domain
    cache: Arc<RwLock<HashMap<String, Arc<ServerConfig>>>>,
}

impl CertificateManager {
    /// Create a new certificate manager
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load or get cached TLS configuration for a domain
    pub async fn get_or_load_config(
        &self,
        domain: &str,
        domain_config: &DomainConfig,
    ) -> Result<Arc<ServerConfig>, IoError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(config) = cache.get(domain) {
                info!("Using cached TLS config for domain: {}", domain);
                return Ok(config.clone());
            }
        }

        // Load from disk if not cached
        info!("Loading TLS config for domain: {}", domain);
        let config = load_tls_config_for_domain(&domain_config.cert_path, &domain_config.key_path)?;
        let config = Arc::new(config);

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(domain.to_string(), config.clone());
        }

        Ok(config)
    }

    /// Clear the certificate cache (useful for certificate rotation)
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Certificate cache cleared");
    }

    /// Build a single ServerConfig handling multiple domains via SNI
    pub fn build_sni_config(domains: &[DomainConfig]) -> Result<Arc<ServerConfig>, IoError> {
        let mut resolver = ResolvesServerCertUsingSni::new();

        let provider = tokio_rustls::rustls::crypto::ring::default_provider();

        for domain_config in domains {
            let certs = load_certs(&domain_config.cert_path)?;
            let key = load_key(&domain_config.key_path)?;

            let signing_key = provider.key_provider.load_private_key(key)
                .map_err(|e| IoError::new(std::io::ErrorKind::InvalidInput, format!("Invalid key for domain {}: {:?}", domain_config.domain, e)))?;

            let certified_key = CertifiedKey::new(certs, signing_key);

            resolver.add(&domain_config.domain, certified_key)
                .map_err(|e| IoError::new(std::io::ErrorKind::InvalidInput, format!("SNI error for {}: {e}", domain_config.domain)))?;

            info!("Added SNI certificate mapping for {}", domain_config.domain);
        }

        let config = ServerConfig::builder_with_provider(Arc::new(provider.clone()))
            .with_safe_default_protocol_versions()
            .map_err(|e| IoError::new(std::io::ErrorKind::Other, format!("Protocol error: {e}")))?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));

        Ok(Arc::new(config))
    }
}

impl Default for CertificateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Load TLS configuration for a single domain
fn load_tls_config_for_domain(cert_path: &str, key_path: &str) -> Result<ServerConfig, IoError> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| IoError::new(std::io::ErrorKind::InvalidInput, format!("TLS config error: {e}")))?;
    Ok(config)
}

/// Load certificates from PEM file
fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, IoError> {
    let certfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(certfile);
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .filter_map(|res| res.ok())
        .collect();
    if certs.is_empty() {
        error!("No certificates found in {path}");
        return Err(IoError::new(std::io::ErrorKind::InvalidInput, "No certificates found"));
    } else {
        info!("{} certificates loaded from {path}", certs.len());
    }
    Ok(certs)
}

/// Load private key from PEM file (supports PKCS#8, RSA, and EC formats)
fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, IoError> {
    let keyfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(keyfile);

    // Try PKCS#8 first
    {
        let mut pkcs8_keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
            .filter_map(|res| res.ok());
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
        let mut rsa_keys = rustls_pemfile::rsa_private_keys(&mut reader)
            .filter_map(|res| res.ok());
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
        let mut ec_keys = rustls_pemfile::ec_private_keys(&mut reader)
            .filter_map(|res| res.ok());
        if let Some(key) = ec_keys.next() {
            info!("EC private key loaded from {path}");
            return Ok(PrivateKeyDer::Sec1(key));
        }
    }

    error!("No private key found in {path} (neither PKCS#8, RSA, nor EC)");
    Err(IoError::new(std::io::ErrorKind::InvalidInput, "No private key found (neither PKCS#8, RSA, nor EC)"))
}
