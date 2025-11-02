use crate::control_system::commands::models::plugin_table::{Column, Table};
use crate::control_system::commands::models::TextMessage;
use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse};
use crate::plugin::PluginManager;
use std::sync::Arc;

pub struct ListPluginsCommand {
    plugin_manager: Arc<PluginManager>,
}

impl ListPluginsCommand {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

impl Command for ListPluginsCommand {
    fn execute(&self, _params: Vec<String>) -> CommandResponse {
        let plugins = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(async { self.plugin_manager.get_all_plugins().await })
        } else {
            match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async { self.plugin_manager.get_all_plugins().await }),
                Err(e) => {
                    return CommandResponse::fail(TextMessage::new(format!(
                        "Failed to create runtime: {}",
                        e
                    )));
                }
            }
        };

        if plugins.is_empty() {
            return CommandResponse::fail(TextMessage::new("No plugins found.".to_string()));
        }

        let columns: Vec<Column> = plugins
            .iter()
            .map(|plugin| {
                Column {
                    name: plugin.config.plugin_name.clone(),
                    state: plugin.state.clone(),
                    protocols: plugin.config.protocols.join(", "),
                    startup_command: plugin.config.startup_command.clone(),
                    max_request_timeout: format!("{}ms", plugin.config.max_request_timeout),
                    request_methods: plugin
                        .config
                        .request_information
                        .request_methods
                        .join(", "),
                    hosts: plugin.config.request_information.hosts.join(", "),
                    paths: plugin.config.request_information.paths.join(", "),
                }
            })
            .collect();

        let table = Table::new(columns);

        CommandResponse::success(table)
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "list-plugins".to_string(),
            "List all plugins with their state and configuration".to_string(),
            vec![],
        )
    }
}
