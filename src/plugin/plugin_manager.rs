use crate::io::data_storage::DataStorage;
use crate::plugin::plugin_config::PluginConfig;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError::PluginScanError;
use crate::plugin_old::Plugin;
use futures::stream;
use futures::StreamExt;
use serde_json::Error;
use std::path::Path;
use tokio::fs;
use tokio::sync::Mutex;

pub enum PluginError {
    PluginScanError(String),
}

pub struct PluginManager {
    pub plugins: Mutex<Vec<Plugin>>,
    plugin_entries: Vec<PluginEntry>,
    data_storage: Box<dyn DataStorage>,
    error_log_path: Box<Path>,
}
impl PluginManager {
    fn new(data_storage: Box<dyn DataStorage>, error_log_path: Box<Path>) -> Self {
        Self {
            plugins: Mutex::new(vec![]),
            plugin_entries: vec![],
            error_log_path,
            data_storage,
        }
    }

    async fn scan_plugins(&mut self, plugins_path: &Path) -> Result<(), PluginError> {
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

                // TODO: double Ok structure seams unclean
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
