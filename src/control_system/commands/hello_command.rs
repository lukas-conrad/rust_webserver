use crate::control_system::commands::models::TextMessage;
use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse, ParameterDescriptor};

pub struct HelloCommand;

impl Command for HelloCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        // First parameter is the name to greet
        let name = params
            .get(0)
            .cloned()
            .unwrap_or_else(|| "World".to_string());

        CommandResponse::new(true, TextMessage::new(format!("hello {}", name)))
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "hello".to_string(),
            "A simple greeting command".to_string(),
            vec![ParameterDescriptor::new(
                "name".to_string(),
                "The name to greet".to_string(),
                false, // optional parameter
            )],
        )
    }
}
