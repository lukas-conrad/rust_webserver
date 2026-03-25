use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Configuration for a single domain with its certificate
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainConfig {
    /// Domain name (used for SNI matching)
    pub domain: String,
    /// Path to SSL certificate file
    pub cert_path: String,
    /// Path to SSL private key file
    pub key_path: String,
}

/// Configuration for HTTP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Whether to enable HTTP server
    pub enabled: bool,
    /// Port for HTTP server
    pub port: u16,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 80,
        }
    }
}

/// Configuration for HTTPS server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpsConfig {
    /// Whether to enable HTTPS server
    pub enabled: bool,
    /// Port for HTTPS server
    pub port: u16,
    /// List of domains with their certificates (for SNI support)
    pub domains: Vec<DomainConfig>,
}

impl Default for HttpsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 443,
            domains: vec![DomainConfig {
                domain: "www.example.com".to_string(),
                cert_path: "path-to-cert".to_string(),
                key_path: "path-to-key".to_string(),
            }],
        }
    }
}

/// Main server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// HTTP configuration
    pub http: HttpConfig,
    /// HTTPS configuration
    pub https: HttpsConfig,
}

impl ServerConfig {
    /// Get default configuration

    /// Load configuration from file, or create default if not found
    pub fn load_or_create(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = Path::new(config_path);

        // Try to load existing config
        if path.exists() {
            info!("Loading configuration from {}", config_path);
            let config_str = fs::read_to_string(path)?;
            let config: ServerConfig = serde_json::from_str(&config_str)?;
            info!("Configuration loaded successfully");
            Ok(config)
        } else {
            // Create default config and save it
            info!(
                "Configuration file not found at {}. Creating default configuration...",
                config_path
            );
            let config = Self::default();
            config.save(config_path)?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self, config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure directory exists
        if let Some(parent) = Path::new(config_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let config_str = serde_json::to_string_pretty(self)?;
        fs::write(config_path, config_str)?;
        info!("Configuration saved to {}", config_path);
        Ok(())
    }
}
