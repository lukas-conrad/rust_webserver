use crate::io::data_storage::DataStorage;
use crate::plugin_old::Plugin;
use std::path::Path;
use tokio::sync::Mutex;

pub enum PluginError {
    PluginScanError,
}

pub struct PluginManager {
    pub plugins: Mutex<Vec<Plugin>>,
    data_storage: Box<dyn DataStorage>,
    error_log_path: Box<Path>,
}
impl PluginManager {
    fn new(data_storage: Box<dyn DataStorage>, error_log_path: Box<Path>) -> Self {
        Self {
            plugins: Mutex::new(vec![]),
            error_log_path,
            data_storage,
        }
    }

    async fn scan_plugins(&self, plugins_path: &Path) -> Result<(), PluginError> {
        
    }
}
