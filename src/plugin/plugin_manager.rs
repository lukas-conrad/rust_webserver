use crate::io::data_storage::DataStorage;
use crate::plugin::plugin_config::PluginConfig;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError::PluginScanError;
use crate::plugin::running_plugin::RunningPlugin;
use crate::plugin_communication::app_starter::plugin_starter::PluginStarter;
use crate::plugin_old::models::HttpRequest;
use crate::plugin_old::models::Package::NormalRequest;
use crate::plugin_old::models::{HttpResponse, NormalRequestContent};
use std::path::Path;
use strum::Display;
use tokio::fs;
use tokio::sync::Mutex;

#[derive(Display, Debug)]
pub enum PluginError {
    PluginScanError(String),
    PluginStartError(String),
    PluginInitError(String),
    PluginNotFoundError(String),
}

pub struct PluginManager {
    pub plugins: Mutex<Vec<RunningPlugin>>,
    pub plugin_entries: Vec<PluginEntry>,
    data_storage: Box<dyn DataStorage>,
    plugin_starter: Box<dyn PluginStarter>,
}
impl PluginManager {
    pub fn new(data_storage: Box<dyn DataStorage>, plugin_starter: Box<dyn PluginStarter>) -> Self {
        Self {
            plugins: Mutex::new(vec![]),
            plugin_entries: vec![],
            data_storage,
            plugin_starter,
        }
    }

    pub async fn route_request(&self, request: HttpRequest) -> Result<HttpResponse, PluginError> {
        let running_plugins = self.plugins.lock().await;
        let plugin = Self::find_plugin_for_request(&*running_plugins, &request).await?;
        let _ = plugin
            .send_package(&NormalRequest(NormalRequestContent {
                package_id: -1,
                http_request: request,
            }))
            .await;

        todo!()
    }

    pub async fn find_plugin_for_request<'a>(
        plugins: &'a [RunningPlugin],
        request: &HttpRequest,
    ) -> Result<&'a RunningPlugin, PluginError> {
        todo!()
    }

    pub async fn start_plugin(&self, plugin_entry: &PluginEntry) -> Result<(), PluginError> {
        let running_plugin =
            RunningPlugin::start_plugin(plugin_entry, &self.plugin_starter).await?;

        self.plugins.lock().await.push(running_plugin);

        Ok(())
    }

    pub async fn scan_plugins(&mut self, plugins_path: &Path) -> Result<(), PluginError> {
        let files = self
            .data_storage
            .list_files(plugins_path, true)
            .await
            .map_err(|e| PluginScanError(e.to_string()))?;
        let mut plugin_entries: Vec<PluginEntry> = vec![];
        for file in files {
            if file.file_name().unwrap() == "plugin_config.json" {
                let config = fs::read_to_string(&file)
                    .await
                    .map(|json| serde_json::from_str::<PluginConfig>(&json));

                // TODO: double Ok structure seems unclean
                if let Ok(Ok(config)) = config {
                    plugin_entries.push(PluginEntry::new(config, file));
                } else {
                    // TODO: add error handling
                }
            }
        }

        self.plugin_entries = plugin_entries;
        Ok(())
    }
}
