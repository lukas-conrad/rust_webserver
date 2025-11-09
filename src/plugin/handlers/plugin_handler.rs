use crate::plugin::handlers::plugin_communicator::AsyncPluginCommunicator;
use crate::plugin::handlers::plugin_handler::PluginError::StartupError;
use crate::plugin::handlers::AsyncPackageHandler;
use crate::plugin::interfaces::{PackageHandlerError, Plugin, PluginCommunicator, State};
use crate::plugin::models;
use crate::plugin::models::{HandshakeRequestContent, HttpRequest, HttpResponse, Package, PackageHandshakeRequest, PackageNormalRequest, PackageShutdownRequest, PackageType, PluginConfig};
use io::ErrorKind;
use rand::random;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin startup failed: {0}")]
    StartupError(String),
}

impl Plugin {
    pub async fn start(
        config_path: Box<PathBuf>,
        callback: Box<dyn Fn(Package, &PluginConfig) + Send + Sync + 'static>,
    ) -> Result<Self, io::Error> {
        let mut file = File::open(&config_path.as_path()).await?;

        let mut string = String::new();
        file.read_to_string(&mut string).await?;

        let config: Arc<PluginConfig> = Arc::new(serde_json::from_str(&string)?);

        let option = config_path.as_path().parent();
        if let None = option {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "No parent directory found",
            ));
        }

        let mut process = Command::new("sh")
            .arg("-c")
            .arg(&config.startup_command)
            .current_dir(option.unwrap())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Extract process streams
        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| io::Error::new(ErrorKind::Other, "Could not extract stdin"))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| io::Error::new(ErrorKind::Other, "Could not extract stdout"))?;

        // Create AsyncPackageHandler
        let package_handler = AsyncPackageHandler::new(stdin, stdout);

        let handler = Box::new(package_handler);
        let config_clone = config.clone();
        let interface = AsyncPluginCommunicator::new(
            config.clone(),
            handler,
            Box::new(move |package| callback(package, &config_clone.clone())),
        ).await;

        let plugin = Self {
            process: Arc::new(Mutex::new(process)),
            state: State::Starting,
            communicator: interface,
            config,
            config_dir: config_path,
            error_callback: None,
        };
        Ok(plugin)
    }

    pub async fn handle_request(
        &self,
        request: HttpRequest,
    ) -> Result<HttpResponse, PackageHandlerError> {
        let package_id = random::<i64>();

        let request_package = PackageNormalRequest {
            package_type: PackageType::NormalRequest,
            content: models::NormalRequestContent {
                package_id,
                http_request: request,
            },
        };

        let response = self.communicator.send_request(request_package).await?;
        Ok(response.content.http_response)
    }

    pub async fn stop(&self) -> Result<(), PackageHandlerError> {
        let request = PackageShutdownRequest {
            package_type: PackageType::ShutdownRequest,
            content: HashMap::new(),
        };
        self.communicator.send_package(request)?;

        // Wait for process termination
        let mut process_guard = self.process.lock().await;
        let status = process_guard.wait().await.map_err(move |e| {
            PackageHandlerError::ProcessCommunicationError(format!(
                "Error when processing request: {}",
                e
            ))
        })?;

        if !status.success() {
            Err(PackageHandlerError::ShutdownError(format!(
                "Process terminated with exit code {}",
                status.code().unwrap_or(-1)
            )))
        } else {
            Ok(())
        }
    }

    pub async fn init(&mut self) -> Result<(), PluginError> {
        let handshake = PackageHandshakeRequest {
            package_type: PackageType::HandshakeRequest,
            content: HandshakeRequestContent {
                protocol: "json".to_string(),
            },
        };
        let result = self.communicator.send_handshake(handshake).await;

        let package = result.map_err(move |e| StartupError(format!("Handshake failed: {}", e)))?;

        if package.content.response_code == 0 {
            self.state = State::Running;
            Ok(())
        } else {
            let string = format!("Plugin rejected handshake {}", package.package_type);
            self.state = State::Error(string.clone());
            Err(StartupError(string))
        }
    }

    pub fn matches_request(&self, method: &str, host: &str, path: &str) -> bool {
        if self.state != State::Running {
            return false;
        }

        let request_info = &self.config.request_information;

        let method_match = request_info
            .request_methods
            .iter()
            .any(|m| m == "*" || m == method);

        let host_match = request_info.hosts.iter().any(|h| {
            if h == "*" {
                true
            } else if h.ends_with("*") {
                let prefix = &h[0..h.len() - 1];
                host.starts_with(prefix)
            } else {
                h == host
            }
        });

        let path_match = request_info.paths.iter().any(|p| {
            if p == "*" {
                true
            } else if p.ends_with("*") {
                let prefix = &p[0..p.len() - 1];
                path.starts_with(prefix)
            } else if p.contains("/**/") {
                // Simple implementation for paths with wildcards
                let parts: Vec<&str> = p.split("/**/").collect();
                path.starts_with(parts[0]) && path.ends_with(parts[1])
            } else {
                p == path
            }
        });

        method_match && host_match && path_match
    }
}
