use crate::control_system::commands::models::TextMessage;
use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse, ParameterDescriptor};
use crate::plugin::PluginManager;
use std::sync::Arc;

pub struct StopPluginCommand {
    plugin_manager: Arc<PluginManager>,
}

impl StopPluginCommand {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

impl Command for StopPluginCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        if params.is_empty() {
            return CommandResponse::fail(TextMessage::new(
                "Missing required parameter: plugin_name (or 'all')".to_string(),
            ));
        }

        let plugin_name = &params[0];

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(async {
                if plugin_name == "all" {
                    self.plugin_manager.stop_all_plugins().await
                } else {
                    self.plugin_manager.stop_plugin(plugin_name).await
                }
            })
        } else {
            match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    if plugin_name == "all" {
                        self.plugin_manager.stop_all_plugins().await
                    } else {
                        self.plugin_manager.stop_plugin(plugin_name).await
                    }
                }),
                Err(e) => {
                    return CommandResponse::fail(TextMessage::new(format!(
                        "Failed to create runtime: {}",
                        e
                    )));
                }
            }
        };

        match result {
            Ok(msg) => CommandResponse::success(TextMessage::new(msg)),
            Err(e) => {
                CommandResponse::fail(TextMessage::new(format!("Failed to stop plugin: {}", e)))
            }
        }
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "stop-plugin",
            "Stop a running plugin or all running plugins",
            vec![ParameterDescriptor::new(
                "plugin_name".to_string(),
                "Name of the plugin to stop, or 'all' to stop all running plugins".to_string(),
                true,
            )],
        )
    }
}
