use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use log::{error, info};

/// Configuration for HTTP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Whether to enable HTTP server
    pub enabled: bool,
    /// Port for HTTP server
    pub port: u16,
}

/// Configuration for HTTPS server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpsConfig {
    /// Whether to enable HTTPS server
    pub enabled: bool,
    /// Port for HTTPS server
    pub port: u16,
    /// Path to SSL certificate file
    pub cert_path: String,
    /// Path to SSL private key file
    pub key_path: String,
}

/// Main server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP configuration
    pub http: HttpConfig,
    /// HTTPS configuration
    pub https: HttpsConfig,
}

impl ServerConfig {
    /// Get default configuration
    pub fn default_config() -> Self {
        ServerConfig {
            http: HttpConfig {
                enabled: true,
                port: 80,
            },
            https: HttpsConfig {
                enabled: false,
                port: 443,
                cert_path: "./certs/server.crt".to_string(),
                key_path: "./certs/server.key".to_string(),
            },
        }
    }

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
            info!("Configuration file not found at {}. Creating default configuration...", config_path);
            let config = Self::default_config();
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

