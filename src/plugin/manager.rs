use crate::plugin::interfaces::{Plugin, PluginCommunicator, PluginError, State};
use crate::plugin::models::PackageContent::{Error, Log};
use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use walkdir::WalkDir;

pub struct PluginManager {
    plugins: Arc<Mutex<Vec<Arc<Plugin>>>>,
    error_log_path: PathBuf,
}

impl PluginManager {
    pub fn new(error_log_path: PathBuf) -> Self {
        PluginManager {
            plugins: Arc::new(Mutex::new(Vec::new())),
            error_log_path,
        }
    }

    pub async fn scan_plugins_directory(&self, plugins_dir: &Path) -> Result<(), PluginError> {
        info!("Scanning plugins directory: {:?}", plugins_dir);

        if !plugins_dir.exists() {
            fs::create_dir_all(plugins_dir).await.map_err(|e| {
                PluginError::ConfigError(format!("Failed to create plugins directory: {}", e))
            })?;
        }

        for entry in WalkDir::new(plugins_dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy() == "pluginConfig.json")
        {
            let config_path = entry.into_path();
            info!("Found plugin config: {:?}", &config_path);

            let plugin = self.start_plugin(config_path).await?;

            self.plugins.lock().await.push(Arc::new(plugin));
        }
        Ok(())
    }

    async fn start_plugin(&self, config_path: PathBuf) -> Result<Plugin, PluginError> {
        let error_log_path = self.error_log_path.clone();

        let mut plugin = Plugin::start(
            Box::new(config_path),
            Box::new(move |package, config| {
                match package.content {
                    Error(content) => {
                        let log_json = serde_json::to_string_pretty(&content).unwrap_or_else(|e| {
                            format!("{{ \"error\": \"Failed to serialize error log: {}\" }}", e)
                        });

                        let timestamp =
                            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

                        let filename = format!("error_{}_{}.json", config.plugin_name, timestamp);
                        let filepath = error_log_path.join(filename);

                        tokio::spawn(async move {
                            if let Err(e) = tokio::fs::write(&filepath, log_json).await {
                                error!("Failed to write error log to file: {}", e);
                            }
                        });
                    }
                    Log(content) => {
                        match content.level.as_str() {
                            "debug" => debug!("[Plugin {}] {}", config.plugin_name, content.message),
                            "info" => info!("[Plugin {}] {}", config.plugin_name, content.message),
                            "warning" => warn!("[Plugin {}] {}", config.plugin_name, content.message),
                            "error" => error!("[Plugin {}] {}", config.plugin_name, content.message),
                            "critical" => error!("[CRITICAL] [Plugin {}] {}", config.plugin_name, content.message),
                            _ => info!("[Plugin {}] {}: {}", config.plugin_name, content.level, content.message),
                        }
                    }
                    _ => {}
                }
            }),
        )
        .await
        .map_err(move |err| PluginError::StartupFailed(format!("Startup failed {}", err)))?;

        plugin
            .init()
            .await
            .map_err(move |err| PluginError::StartupFailed(format!("Startup failed {}", err)))?;

        Ok(plugin)
    }

    pub async fn get_active_plugins(&self) -> Vec<Arc<Plugin>> {
        let plugins = self.plugins.lock().await;
        plugins
            .iter()
            .filter(|p| p.state == State::Running)
            .cloned()
            .collect()
    }

    pub async fn get_plugin(&self, plugin_name: &str) -> Option<Arc<Plugin>> {
        let plugins = self.plugins.lock().await;
        plugins
            .iter()
            .find(|p| p.config.plugin_name == plugin_name && p.state == State::Running)
            .cloned()
    }

}
