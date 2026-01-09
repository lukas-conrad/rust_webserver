use crate::plugin::plugin_config::PluginConfig;
use std::path::Path;

pub struct PluginEntry {
    config: PluginConfig,
    path: Box<Path>,
}

impl PluginEntry {
    pub(crate) fn new(config: PluginConfig, path: Box<Path>) -> Self {
        Self { config, path }
    }
}
