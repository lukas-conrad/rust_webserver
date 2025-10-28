use super::commands::{HelloCommand, HelpCommand};
use super::models::{CommandDescriptor, CommandRequest, CommandResponse};
use std::collections::HashMap;

pub trait ControlSystem {
    fn run_command(&self, request: CommandRequest) -> CommandResponse;
}

pub trait Command: Send + Sync {
    fn execute(&self, params: Vec<String>) -> CommandResponse;
    fn get_command_descriptor(&self) -> CommandDescriptor;
}

pub struct DefaultControlSystem {
    commands: HashMap<String, Box<dyn Command>>,
}

impl DefaultControlSystem {
    pub fn new() -> Self {
        let mut system = Self {
            commands: HashMap::new(),
        };

        system.register_command(Box::new(HelloCommand));
        system.register_command(Box::new(HelpCommand::new(system.get_all_command_descriptors())));

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
            None => CommandResponse::new(false, format!("Command '{}' not found", request.name)),
        }
    }
}
