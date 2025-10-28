use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse, ParameterDescriptor};
use crate::plugin::PluginManager;
use crate::plugin::interfaces::State;
use std::sync::Arc;

pub struct HelloCommand;

impl Command for HelloCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        // Erster Parameter ist der Name zum Begrüßen
        let name = params.get(0)
            .cloned()
            .unwrap_or_else(|| "World".to_string());

        CommandResponse::new(true, format!("hello {}", name))
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "hello".to_string(),
            "A simple greeting command".to_string(),
            vec![
                ParameterDescriptor::new(
                    "name".to_string(),
                    "The name to greet".to_string(),
                    false, // optional parameter
                )
            ],
        )
    }
}

pub struct HelpCommand {
    command_descriptors: Vec<CommandDescriptor>,
}

impl HelpCommand {
    pub fn new(command_descriptors: Vec<CommandDescriptor>) -> Self {
        Self {
            command_descriptors,
        }
    }
}

impl Command for HelpCommand {
    fn execute(&self, _params: Vec<String>) -> CommandResponse {
        let descriptors = &self.command_descriptors;

        let mut help_text = String::from("\n=== Available Commands ===\n\n");

        for descriptor in descriptors {
            // Überspringe den help command selbst in der Ausgabe (um Rekursion zu vermeiden)
            if descriptor.name == "help" {
                continue;
            }

            help_text.push_str(&format!("• {}\n", descriptor.name));
            help_text.push_str(&format!("  Description: {}\n", descriptor.description));

            if !descriptor.parameters.is_empty() {
                help_text.push_str("  Parameters:\n");
                for (index, param) in descriptor.parameters.iter().enumerate() {
                    let required_text = if param.required { "required" } else { "optional" };
                    help_text.push_str(&format!(
                        "    {} [{}] {} - {}\n",
                        index + 1,
                        required_text,
                        param.name,
                        param.description
                    ));
                }
            }
            help_text.push('\n');
        }

        help_text.push_str("=========================\n");
        help_text.push_str("\nUsage: <command> <arg1> <arg2> ...\n");

        CommandResponse::new(true, help_text)
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "help".to_string(),
            "Display all available commands and their descriptions".to_string(),
            vec![],
        )
    }
}

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
            handle.block_on(async {
                self.plugin_manager.get_all_plugins().await
            })
        } else {
            match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    self.plugin_manager.get_all_plugins().await
                }),
                Err(e) => {
                    return CommandResponse::new(
                        false,
                        format!("Failed to create runtime: {}", e)
                    );
                }
            }
        };

        if plugins.is_empty() {
            return CommandResponse::new(true, "No plugins found.".to_string());
        }

        let mut output = String::from("\n=== Plugin List ===\n\n");

        for (index, plugin) in plugins.iter().enumerate() {
            let state_str = match &plugin.state {
                State::Running => "Running",
                State::Starting => "Starting",
                State::Stopped => "Stopped",
                State::Error(err) => &format!("Error: {}", err),
            };

            output.push_str(&format!("{}. {}\n", index + 1, plugin.config.plugin_name));
            output.push_str(&format!("   State: {}\n", state_str));
            output.push_str(&format!("   Protocols: {}\n", plugin.config.protocols.join(", ")));
            output.push_str(&format!("   Startup Command: {}\n", plugin.config.startup_command));
            output.push_str(&format!("   Max Request Timeout: {}ms\n", plugin.config.max_request_timeout));
            output.push_str(&format!("   Request Methods: {}\n", plugin.config.request_information.request_methods.join(", ")));
            output.push_str(&format!("   Hosts: {}\n", plugin.config.request_information.hosts.join(", ")));
            output.push_str(&format!("   Paths: {}\n", plugin.config.request_information.paths.join(", ")));
            output.push('\n');
        }

        output.push_str(&format!("Total: {} plugin(s)\n", plugins.len()));
        output.push_str("===================\n");

        CommandResponse::new(true, output)
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "list-plugins".to_string(),
            "List all plugins with their state and configuration".to_string(),
            vec![],
        )
    }
}

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
            return CommandResponse::new(
                false,
                "Missing required parameter: plugin_name (or 'all')".to_string(),
            );
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
                    return CommandResponse::new(false, format!("Failed to create runtime: {}", e));
                }
            }
        };

        match result {
            Ok(msg) => CommandResponse::new(true, msg),
            Err(e) => CommandResponse::new(false, format!("Failed to stop plugin: {}", e)),
        }
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "stop-plugin".to_string(),
            "Stop a running plugin or all running plugins".to_string(),
            vec![ParameterDescriptor::new(
                "plugin_name".to_string(),
                "Name of the plugin to stop, or 'all' to stop all running plugins".to_string(),
                true,
            )],
        )
    }
}

pub struct StartPluginCommand {
    plugin_manager: Arc<PluginManager>,
}

impl StartPluginCommand {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

impl Command for StartPluginCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        if params.is_empty() {
            return CommandResponse::new(
                false,
                "Missing required parameter: plugin_name (or 'all')".to_string(),
            );
        }

        let plugin_name = &params[0];

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(async {
                if plugin_name == "all" {
                    self.plugin_manager.start_all_plugins().await
                } else {
                    self.plugin_manager.start_plugin_by_name(plugin_name).await
                }
            })
        } else {
            match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    if plugin_name == "all" {
                        self.plugin_manager.start_all_plugins().await
                    } else {
                        self.plugin_manager.start_plugin_by_name(plugin_name).await
                    }
                }),
                Err(e) => {
                    return CommandResponse::new(false, format!("Failed to create runtime: {}", e));
                }
            }
        };

        match result {
            Ok(msg) => CommandResponse::new(true, msg),
            Err(e) => CommandResponse::new(false, format!("Failed to start plugin: {}", e)),
        }
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "start-plugin".to_string(),
            "Start a stopped plugin or all stopped/error plugins".to_string(),
            vec![ParameterDescriptor::new(
                "plugin_name".to_string(),
                "Name of the plugin to start, or 'all' to start all stopped/error plugins".to_string(),
                true,
            )],
        )
    }
}

pub struct ReloadPluginCommand {
    plugin_manager: Arc<PluginManager>,
}

impl ReloadPluginCommand {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self { plugin_manager }
    }
}

impl Command for ReloadPluginCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        if params.is_empty() {
            return CommandResponse::new(
                false,
                "Missing required parameter: plugin_name (or 'all')".to_string(),
            );
        }

        let plugin_name = &params[0];

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(async {
                if plugin_name == "all" {
                    self.plugin_manager.reload_all_plugins().await
                } else {
                    self.plugin_manager.reload_plugin(plugin_name).await
                }
            })
        } else {
            match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    if plugin_name == "all" {
                        self.plugin_manager.reload_all_plugins().await
                    } else {
                        self.plugin_manager.reload_plugin(plugin_name).await
                    }
                }),
                Err(e) => {
                    return CommandResponse::new(false, format!("Failed to create runtime: {}", e));
                }
            }
        };

        match result {
            Ok(msg) => CommandResponse::new(true, msg),
            Err(e) => CommandResponse::new(false, format!("Failed to reload plugin: {}", e)),
        }
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "reload-plugin".to_string(),
            "Reload a plugin (stop and start) or all plugins".to_string(),
            vec![ParameterDescriptor::new(
                "plugin_name".to_string(),
                "Name of the plugin to reload, or 'all' to reload all plugins".to_string(),
                true,
            )],
        )
    }
}

