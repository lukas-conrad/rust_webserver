use super::commands::{
    HelloCommand, HelpCommand, ListPluginsCommand, ReloadPluginCommand, StartPluginCommand,
    StopPluginCommand,
};
use super::models::{CommandDescriptor, CommandRequest, CommandResponse};
use crate::control_system::commands::models::TextMessage;
use crate::plugin::PluginManager;
use std::collections::HashMap;
use std::sync::Arc;
use strum::Display;
use thiserror::Error;

pub trait ControlSystem {
    fn run_command(&self, request: CommandRequest) -> CommandResponse;
}

pub trait Command: Send + Sync {
    fn execute(&self, params: Vec<String>) -> CommandResponse;
    fn get_command_descriptor(&self) -> CommandDescriptor;
}

#[derive(Debug, Display, Error)]
pub enum CommandError {
    ParseError(String),
}

#[macro_export]
macro_rules! param {
    (
        $name:ident {
            $(($field:ident : $ty:ty, $desc:literal, $required:literal, $named:literal)),* $(,)?
        }
    ) => {
        #[derive(Default)]
        struct $name {
            $($field: $ty),*
        }

        impl $name {

            #[allow(unused_assignments)]
            fn parse(params: Vec<String>) -> Result<Self, crate::control_system::control_system::CommandError> {
                let mut iter = params.iter();
                let mut index = 0;

                $(
                    let $field = if let Some(value_str) = iter.next() {
                        value_str.parse::<$ty>()
                            .map_err(|_| crate::control_system::control_system::CommandError::ParseError(
                                format!("Failed to parse parameter '{}' at position {}: '{}'",
                                    stringify!($field), index, value_str)
                            ))?
                    } else {
                        let is_required = $required;
                        if is_required {
                            return Err(crate::control_system::control_system::CommandError::ParseError(
                                format!("Required parameter '{}' at position {} is missing",
                                    stringify!($field), index)
                            ));
                        }
                        <$ty>::default()
                    };
                    index += 1;
                )*

                Ok(Self {
                    $($field),*
                })
            }

            fn param_description() -> Vec<ParameterDescriptor> {
                vec![
                    $(ParameterDescriptor::new(
                        stringify!($field).to_string(),
                        $desc.to_string(),
                        $required,
                    ),)*
                ]
            }
        }

        const _: () = {
            fn assert_from_str<T: std::str::FromStr>() {}

            #[allow(dead_code)]
            fn check_traits() {
                $(
                    assert_from_str::<$ty>();
                )*
            }
        };
    };
}

pub struct DefaultControlSystem {
    commands: HashMap<String, Box<dyn Command>>,
}

impl DefaultControlSystem {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        let mut system = Self {
            commands: HashMap::new(),
        };

        system.register_command(Box::new(HelloCommand));
        system.register_command(Box::new(ListPluginsCommand::new(plugin_manager.clone())));
        system.register_command(Box::new(StopPluginCommand::new(plugin_manager.clone())));
        system.register_command(Box::new(StartPluginCommand::new(plugin_manager.clone())));
        system.register_command(Box::new(ReloadPluginCommand::new(plugin_manager)));
        system.register_command(Box::new(HelpCommand::new(
            system.get_all_command_descriptors(),
        )));

        system
    }

    pub fn register_command(&mut self, command: Box<dyn Command>) {
        let descriptor = command.get_command_descriptor();
        self.commands.insert(descriptor.name.clone(), command);
    }

    pub fn get_all_command_descriptors(&self) -> Vec<CommandDescriptor> {
        self.commands
            .values()
            .map(|cmd| cmd.get_command_descriptor())
            .collect()
    }
}

impl ControlSystem for DefaultControlSystem {
    fn run_command(&self, request: CommandRequest) -> CommandResponse {
        match self.commands.get(&request.name) {
            Some(command) => command.execute(request.args),
            None => CommandResponse::fail(TextMessage::new(format!(
                "Command '{}' not found",
                request.name
            ))),
        }
    }
}
