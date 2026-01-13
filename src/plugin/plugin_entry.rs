use crate::plugin::plugin_config::PluginConfig;
use std::path::Path;

pub struct PluginEntry {
    pub config: PluginConfig,
    pub path: Box<Path>,
}

impl PluginEntry {
    pub fn new(config: PluginConfig, path: Box<Path>) -> Self {
        Self { config, path }
    }
}
