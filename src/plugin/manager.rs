use std::ops::Deref;
use crate::control_system::control_system::ControlSystem;
use crate::plugin::interfaces::{Plugin, PluginCommunicator, PluginError, State};
use crate::plugin::models::Package::{CliRequest, CliResponse, Error, Log};
use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use walkdir::WalkDir;
use crate::plugin::handlers::plugin_communicator::AsyncPluginCommunicator;
use crate::plugin::models::CliResponseContent;
use crate::plugin::PackageHandler;

pub struct PluginManager {
    plugins: Arc<Mutex<Vec<Arc<Plugin>>>>,
    error_log_path: PathBuf,
    pub cli: Arc<Mutex<Option<Arc<dyn ControlSystem + Send + Sync>>>>,
}

impl PluginManager {
    pub fn new(error_log_path: PathBuf) -> Self {
        PluginManager {
            plugins: Arc::new(Mutex::new(Vec::new())),
            error_log_path,
            cli: Arc::new(Mutex::new(None)),
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

            self.plugins.lock().await.push(plugin);
        }
        Ok(())
    }

    async fn start_plugin(&self, config_path: PathBuf) -> Result<Arc<Plugin>, PluginError> {
        let error_log_path = self.error_log_path.clone();
        let cli = self.cli.clone();

        let plugin_clone: Arc<Mutex<Option<Arc<Plugin>>>> = Arc::new(Mutex::new(None));
        let plugin_clone2 = plugin_clone.clone();
        let mut plugin = Plugin::start(
            Box::new(config_path),
            Box::new(move |package, config| {
                match package {
                    Error(content) => {
                        let log_json = serde_json::to_string_pretty(&content).unwrap_or_else(|e| {
                            format!("{{ \"error\": \"Failed to serialize error log: {}\" }}", e)
                        });

                        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

                        let filename = format!("error_{}_{}.json", config.plugin_name, timestamp);
                        let filepath = error_log_path.join(filename);

                        tokio::spawn(async move {
                            if let Err(e) = tokio::fs::write(&filepath, log_json).await {
                                error!("Failed to write error log to file: {}", e);
                            }
                        });
                    }
                    Log(content) => match content.level.as_str() {
                        "debug" => debug!("[Plugin {}] {}", config.plugin_name, content.message),
                        "info" => info!("[Plugin {}] {}", config.plugin_name, content.message),
                        "warning" => warn!("[Plugin {}] {}", config.plugin_name, content.message),
                        "error" => error!("[Plugin {}] {}", config.plugin_name, content.message),
                        "critical" => error!(
                        "[CRITICAL] [Plugin {}] {}",
                        config.plugin_name, content.message
                    ),
                        _ => info!(
                        "[Plugin {}] {}: {}",
                        config.plugin_name, content.level, content.message
                    ),
                    },
                    CliRequest(request) => {
                        let cli_clone = cli.clone();
                        let plugin_clone = plugin_clone2.clone();
                        tokio::spawn(async move {
                            let plugin_guard = plugin_clone.lock().await;

                            if let Some(control_system) = cli_clone.lock().await.as_deref() {
                                if let Some(communicator) = plugin_guard.as_ref() {
                                    let response = control_system.run_command(request);
                                    communicator.communicator.send_package(CliResponse(CliResponseContent {
                                        success: response.success,
                                        response: response.message.to_json()
                                    })).expect("Failed to send CLI response");
                                }
                            }
                        });
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

        let plugin_arc = Arc::new(plugin);
        *plugin_clone.lock().await = Some(plugin_arc.clone());

        Ok(plugin_arc)
    }

    pub async fn get_active_plugins(&self) -> Vec<Arc<Plugin>> {
        let plugins = self.plugins.lock().await;
        plugins
            .iter()
            .filter(|p| p.state == State::Running)
            .cloned()
            .collect()
    }

    pub async fn get_all_plugins(&self) -> Vec<Arc<Plugin>> {
        let plugins = self.plugins.lock().await;
        plugins.clone()
    }

    pub async fn get_plugin(&self, plugin_name: &str) -> Option<Arc<Plugin>> {
        let plugins = self.plugins.lock().await;
        plugins
            .iter()
            .find(|p| p.config.plugin_name == plugin_name && p.state == State::Running)
            .cloned()
    }

    pub async fn stop_plugin(&self, plugin_name: &str) -> Result<String, PluginError> {
        let plugins = self.plugins.lock().await;

        let plugin = plugins
            .iter()
            .find(|p| p.config.plugin_name == plugin_name)
            .ok_or_else(|| {
                PluginError::ConfigError(format!("Plugin '{}' not found", plugin_name))
            })?;

        // Nur stoppen, wenn das Plugin läuft
        if plugin.state != State::Running {
            return Err(PluginError::ProcessError(format!(
                "Plugin '{}' is not running (current state: {:?})",
                plugin_name, plugin.state
            )));
        }

        // Plugin stoppen (bleibt aber in der Liste)
        plugin
            .stop()
            .await
            .map_err(|e| PluginError::ProcessError(format!("Failed to stop plugin: {}", e)))?;

        info!("Plugin '{}' stopped successfully", plugin_name);
        Ok(format!("Plugin '{}' stopped successfully", plugin_name))
    }

    pub async fn start_plugin_by_name(&self, plugin_name: &str) -> Result<String, PluginError> {
        let mut plugins = self.plugins.lock().await;

        // Finde das Plugin in der Liste
        let plugin_index = plugins
            .iter()
            .position(|p| p.config.plugin_name == plugin_name)
            .ok_or_else(|| {
                PluginError::ConfigError(format!(
                    "Plugin '{}' not found in plugin list",
                    plugin_name
                ))
            })?;

        let existing_plugin = &plugins[plugin_index];

        // Prüfe, ob das Plugin bereits läuft
        if existing_plugin.state == State::Running {
            return Err(PluginError::ProcessError(format!(
                "Plugin '{}' is already running",
                plugin_name
            )));
        }

        // Hole den Config-Pfad vom existierenden Plugin
        let config_path = existing_plugin.config_dir.join("pluginConfig.json");

        // Entferne das alte Plugin und starte ein neues
        // TODO: Es soll das Plugin nicht removen alla
        plugins.remove(plugin_index);
        drop(plugins); // Release lock before starting plugin

        // Starte das Plugin neu
        let new_plugin = self.start_plugin(config_path).await?;
        self.plugins.lock().await.push(new_plugin);

        info!("Plugin '{}' started successfully", plugin_name);
        Ok(format!("Plugin '{}' started successfully", plugin_name))
    }

    pub async fn reload_plugin(&self, plugin_name: &str) -> Result<String, PluginError> {
        // Stoppe das Plugin
        self.stop_plugin(plugin_name).await?;

        // Starte das Plugin neu
        self.start_plugin_by_name(plugin_name).await?;

        info!("Plugin '{}' reloaded successfully", plugin_name);
        Ok(format!("Plugin '{}' reloaded successfully", plugin_name))
    }

    pub async fn stop_all_plugins(&self) -> Result<String, PluginError> {
        let plugins_to_stop: Vec<String> = {
            let plugins = self.plugins.lock().await;
            plugins
                .iter()
                .filter(|p| p.state == State::Running)
                .map(|p| p.config.plugin_name.clone())
                .collect()
        };

        let mut stopped_count = 0;
        let mut errors = Vec::new();

        for plugin_name in plugins_to_stop {
            match self.stop_plugin(&plugin_name).await {
                Ok(_) => stopped_count += 1,
                Err(e) => errors.push(format!("{}: {}", plugin_name, e)),
            }
        }

        if errors.is_empty() {
            Ok(format!("Stopped {} plugin(s) successfully", stopped_count))
        } else {
            Err(PluginError::ProcessError(format!(
                "Stopped {} plugin(s), but {} failed: {}",
                stopped_count,
                errors.len(),
                errors.join(", ")
            )))
        }
    }

    pub async fn start_all_plugins(&self) -> Result<String, PluginError> {
        let plugins_to_start: Vec<String> = {
            let plugins = self.plugins.lock().await;
            plugins
                .iter()
                .filter(|p| p.state != State::Running)
                .map(|p| p.config.plugin_name.clone())
                .collect()
        };

        let mut started_count = 0;
        let mut errors = Vec::new();

        for plugin_name in plugins_to_start {
            match self.start_plugin_by_name(&plugin_name).await {
                Ok(_) => started_count += 1,
                Err(e) => errors.push(format!("{}: {}", plugin_name, e)),
            }
        }

        if errors.is_empty() {
            Ok(format!("Started {} plugin(s) successfully", started_count))
        } else {
            Ok(format!(
                "Started {} plugin(s), {} failed: {}",
                started_count,
                errors.len(),
                errors.join(", ")
            ))
        }
    }

    pub async fn reload_all_plugins(&self) -> Result<String, PluginError> {
        self.stop_all_plugins().await?;
        self.start_all_plugins().await
    }
}
