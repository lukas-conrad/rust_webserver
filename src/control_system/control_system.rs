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
    (@field_type $ty:ty, true) => { $ty };
    (@field_type $ty:ty, false) => { Option<$ty> };
    
    (@unwrap_value $value:expr, true) => { $value.unwrap_or_default() };
    (@unwrap_value $value:expr, false) => { $value };
    
    (
        $name:ident {
            positional: [
                $(($pos_field:ident : $pos_ty:ty, $pos_desc:literal)),* $(,)?
            ],
            named: [
                $(($named_field:ident : $named_ty:ty, $named_desc:literal, $named_required:tt, $named_matcher:expr, $named_parser:expr)),* $(,)?
            ],
            flags: [
                $(($flag_field:ident : $flag_ty:ty, $flag_desc:literal, $flag_matcher:expr)),* $(,)?
            ]
        }
    ) => {
        #[derive(Default)]
        struct $name {
            $($pos_field: $pos_ty,)*
            $($named_field: param!(@field_type $named_ty, $named_required),)*
            $($flag_field: $flag_ty,)*
        }

        impl $name {
            #[allow(unused_assignments, unused_mut, unused_variables)]
            fn parse(params: Vec<String>) -> Result<Self, crate::control_system::control_system::CommandError> {
                let mut params_iter = params.iter();
                let mut positional_index = 0;

                // Parse positional parameters first
                $(
                    let $pos_field = if let Some(value_str) = params_iter.next() {
                        value_str.parse::<$pos_ty>()
                            .map_err(|_| crate::control_system::control_system::CommandError::ParseError(
                                format!("Failed to parse positional parameter '{}' at position {}: '{}'",
                                    stringify!($pos_field), positional_index, value_str)
                            ))?
                    } else {
                        return Err(crate::control_system::control_system::CommandError::ParseError(
                            format!("Required positional parameter '{}' at position {} is missing",
                                stringify!($pos_field), positional_index)
                        ));
                    };
                    positional_index += 1;
                )*

                // Initialize named and flag parameters with defaults
                $(let mut $named_field: Option<$named_ty> = None;)*
                $(let mut $flag_field: $flag_ty = false;)*

                // Parse remaining parameters as named or flags
                for param_str in params_iter {
                    let mut matched = false;

                    // Try to match named parameters
                    $(
                        if !matched {
                            let matcher: fn(&str) -> bool = $named_matcher;
                            if matcher(param_str) {
                                let parser: fn(&str) -> Result<$named_ty, String> = $named_parser;
                                match parser(param_str) {
                                    Ok(value) => {
                                        $named_field = Some(value);
                                        matched = true;
                                    }
                                    Err(err_msg) => {
                                        return Err(crate::control_system::control_system::CommandError::ParseError(
                                            format!("Failed to parse named parameter '{}': {}",
                                                stringify!($named_field), err_msg)
                                        ));
                                    }
                                }
                            }
                        }
                    )*

                    // Try to match flags
                    $(
                        if !matched {
                            let matcher: fn(&str) -> bool = $flag_matcher;
                            if matcher(param_str) {
                                $flag_field = true;
                                matched = true;
                            }
                        }
                    )*

                    if !matched {
                        return Err(crate::control_system::control_system::CommandError::ParseError(
                            format!("Unknown parameter: '{}'", param_str)
                        ));
                    }
                }

                // Check required named parameters
                $(
                    let is_required = $named_required;
                    if is_required && $named_field.is_none() {
                        return Err(crate::control_system::control_system::CommandError::ParseError(
                            format!("Required named parameter '{}' is missing", stringify!($named_field))
                        ));
                    }
                )*

                Ok(Self {
                    $($pos_field,)*
                    $($named_field: param!(@unwrap_value $named_field, $named_required),)*
                    $($flag_field,)*
                })
            }

            #[allow(unused_assignments, unused_mut, unused_variables)]
            fn param_description() -> Vec<ParameterDescriptor> {
                let mut params = Vec::new();
                let mut positional_index = 0;
                
                $(
                    params.push(ParameterDescriptor::new(
                        format!("[{}] {}", positional_index, stringify!($pos_field)),
                        $pos_desc.to_string(),
                        true,
                    ));
                    positional_index += 1;
                )*
                
                $(
                    params.push(ParameterDescriptor::new(
                        format!("--{}", stringify!($named_field)),
                        $named_desc.to_string(),
                        $named_required,
                    ));
                )*
                
                $(
                    params.push(ParameterDescriptor::new(
                        format!("--{}", stringify!($flag_field)),
                        $flag_desc.to_string(),
                        false,
                    ));
                )*
                
                params
            }
        }

        const _: () = {
            fn assert_from_str<T: std::str::FromStr>() {}

            #[allow(dead_code)]
            fn check_traits() {
                $(
                    assert_from_str::<$pos_ty>();
                )*
                $(
                    assert_from_str::<$named_ty>();
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
