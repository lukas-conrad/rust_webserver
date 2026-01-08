use crate::plugin::plugin_config::PluginConfig;
use std::path::Path;

struct PluginEntry {
    config: PluginConfig,
    path: Box<Path>,
}

impl PluginEntry {
    fn new(config: PluginConfig, path: Box<Path>) -> Self {
        Self { config, path }
    }
}
