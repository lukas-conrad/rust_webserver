use crate::config::DomainConfig;
use crate::file_watcher::FileWatcher;
use log::{error, info};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::server::ResolvesServerCertUsingSni;
use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

pub struct CertificateManager {}

impl CertificateManager {
    /// Build a single ServerConfig handling multiple domains via SNI
    pub async fn create_updating_acceptor(
        domains: &[DomainConfig],
    ) -> Result<Arc<RwLock<TlsAcceptor>>, Box<dyn Error + Send + Sync>> {
        let config = Self::create_sni_resolver(domains)?;

        let acceptor = Arc::new(RwLock::new(TlsAcceptor::from(Arc::new(config))));

        let paths = domains
            .iter()
            .map(|cfg| vec![cfg.cert_path.clone(), cfg.key_path.clone()])
            .flatten()
            .map(|path| PathBuf::from(path))
            .collect();

        let domains = domains.to_vec();
        let mut watcher = cloned!(acceptor, domains; FileWatcher::new(paths, Arc::new(move |_| {

            match Self::create_sni_resolver(domains.as_slice()) {
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
        domains: &[DomainConfig],
    ) -> Result<ServerConfig, Box<dyn Error + Send + Sync>> {
        let mut resolver = ResolvesServerCertUsingSni::new();

        let provider = tokio_rustls::rustls::crypto::ring::default_provider();

        for domain_config in domains {
            let certs = load_certs(&domain_config.cert_path)?;
            let key = load_key(&domain_config.key_path)?;

            let signing_key = provider.key_provider.load_private_key(key).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid key for domain {}: {:?}", domain_config.domain, e),
                )
            })?;

            let certified_key = CertifiedKey::new(certs, signing_key);

            resolver
                .add(&domain_config.domain, certified_key)
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("SNI error for {}: {e}", domain_config.domain),
                    )
                })?;

            info!("Added SNI certificate mapping for {}", domain_config.domain);
        }

        let config = ServerConfig::builder_with_provider(Arc::new(provider.clone()))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));
        Ok(config)
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
