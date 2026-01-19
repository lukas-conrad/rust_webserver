use crate::io::data_storage::DataStorage;
use crate::plugin::plugin_config::PluginConfig;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError::{PluginNotFoundError, PluginScanError};
use crate::plugin::running_plugin::RunningPlugin;
use crate::plugin_communication::app_starter::plugin_starter::PluginStarter;
use crate::plugin_old::models::HttpRequest;
use crate::plugin_old::models::Package::{NormalRequest, NormalResponse};
use crate::plugin_old::models::{HttpResponse, NormalRequestContent};
use async_trait::async_trait;
use log::{debug, error, info};
use std::path::Path;
use strum::Display;
use tokio::sync::Mutex;

#[derive(Display, Debug)]
pub enum PluginError {
    PluginScanError(String),
    PluginStartError(String),
    PluginInitError(String),
    PluginCommunicationError(String),
    PluginNotFoundError(String),
}

#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn route_request(&self, request: HttpRequest) -> Result<HttpResponse, PluginError>;
}
pub struct PluginManager {
    pub plugins: Mutex<Vec<RunningPlugin>>,
    pub plugin_entries: Vec<PluginEntry>,
    data_storage: Mutex<Box<dyn DataStorage>>,
    plugin_starter: Box<dyn PluginStarter>,
}
impl PluginManager {
    pub fn new(data_storage: Box<dyn DataStorage>, plugin_starter: Box<dyn PluginStarter>) -> Self {
        Self {
            plugins: Mutex::new(vec![]),
            plugin_entries: vec![],
            data_storage: Mutex::new(data_storage),
            plugin_starter,
        }
    }

    pub fn find_plugin_for_request<'a>(
        plugins: &'a [RunningPlugin],
        request: &HttpRequest,
    ) -> Option<&'a RunningPlugin> {
        let plugin = plugins
            .iter()
            .map(|plugin| {
                (
                    plugin,
                    plugin
                        .entry
                        .match_count(&request.host, &request.path, &request.request_method),
                )
            })
            .max_by(|(_, specificity_1), (_, specificity_2)| specificity_1.cmp(&specificity_2));

        if let Some((plugin, _)) = plugin {
            Some(plugin)
        } else {
            None
        }
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
            .lock()
            .await
            .list_files(plugins_path, true)
            .await
            .map_err(|e| PluginScanError(e.to_string()))?;
        let mut plugin_entries: Vec<PluginEntry> = vec![];
        info!("Searching in {} files", plugin_entries.len());
        for file in files {
            if file.file_name().unwrap() == "plugin_config.json" {
                debug!("Found Plugin Config at {:?}", file.to_str());
                let config = async {
                    let data = self
                        .data_storage
                        .lock()
                        .await
                        .load_data(&file)
                        .await
                        .map_err(|_| PluginScanError("File could not be loaded".to_string()))?;
                    let json = String::from_utf8(data).map_err(|_| {
                        PluginScanError("File could not be loaded as UTF-8".to_string())
                    })?;
                    serde_json::from_str::<PluginConfig>(&json).map_err(|_| {
                        PluginScanError("File could not be parsed as JSON".to_string())
                    })
                }
                .await;

                match config {
                    Ok(config) => {
                        plugin_entries.push(PluginEntry::new(config, file));
                    }
                    Err(err) => {
                        error!("Plugin config could not be loaded ({})", err);
                    }
                }
            }
        }

        self.plugin_entries = plugin_entries;
        Ok(())
    }
}

#[async_trait]
impl RequestHandler for PluginManager {
    async fn route_request(&self, request: HttpRequest) -> Result<HttpResponse, PluginError> {
        let running_plugins = self.plugins.lock().await;
        let plugin = Self::find_plugin_for_request(&*running_plugins, &request);

        let plugin = plugin.ok_or(PluginNotFoundError(
            "Could not find any plugin to match this request".to_string(),
        ))?;
        let package_id = rand::random();

        let request = NormalRequest(NormalRequestContent {
            package_id,
            http_request: request,
        });
        let NormalResponse(response) = plugin
            .send_package_with_response(
                &request,
                Box::new(move |package| {
                    if let NormalResponse(content) = package {
                        return content.package_id == package_id;
                    }
                    return false;
                }),
            )
            .await
            .map_err(|e| PluginError::PluginCommunicationError(e.to_string()))?
        else {
            panic!("Wrong package returned")
        };

        Ok(response.http_response)
    }
}
