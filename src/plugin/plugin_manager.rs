use crate::io::data_storage::DataStorage;
use crate::plugin::plugin_config::PluginConfig;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError::PluginScanError;
use crate::plugin::running_plugin::RunningPlugin;
use crate::plugin_communication::app_starter::plugin_starter::PluginStarter;
use crate::plugin_old::models::HttpRequest;
use crate::plugin_old::models::Package::{NormalRequest, NormalResponse};
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
    PluginCommunicationError(String),
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
        let package_id = rand::random();
        let NormalResponse(response) = plugin
            .send_package_with_response(
                &NormalRequest(NormalRequestContent {
                    package_id,
                    http_request: request,
                }),
                Box::new(move |package| {
                    if let NormalRequest(content) = package {
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

    pub async fn find_plugin_for_request<'a>(
        plugins: &'a [RunningPlugin],
        request: &HttpRequest,
    ) -> Result<&'a RunningPlugin, PluginError> {
        let matches: Vec<_> = plugins
            .iter()
            .filter(|plugin| {
                let plugin_methods = &plugin.entry.config.request_information.request_methods;

                plugin_methods.contains(&"*".to_string())
                    || plugin_methods.contains(&request.request_method)
            })
            .filter(|plugin| {
                let plugin_hosts = &plugin.entry.config.request_information.hosts;

                false
            })
            .map(|x| {})
            .collect();
        for plugin in plugins {
            let plugin_methods = &plugin.entry.config.request_information.request_methods;
            let plugin_paths = &plugin.entry.config.request_information.paths;
            let plugin_hosts = &plugin.entry.config.request_information.hosts;
        }

        todo!()
    }

    fn matches(actual: String, pattern: String) -> bool {
        let mut pattern_pointer: usize = 0;
        for i in 0..actual.len() {
            let actual_letter = &actual[i..i + 1];
            let pattern_letter = &pattern[pattern_pointer..pattern_pointer + 1];
            let next_pattern_letter = &pattern[pattern_pointer + 1..pattern_pointer + 2];
            if pattern_letter == "*" {
                if actual_letter != pattern_letter {

                }
            } else if actual_letter == pattern_letter {
                pattern_pointer += 1;
            } else {
                return false;
            }
        }

        true
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
