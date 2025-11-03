use crate::control_system::commands::models::TextMessage;
use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse, ParameterDescriptor};
use crate::param;

pub struct HelloCommand;

param! {
    WorldInput {
        (name: String, "The name to greet", false, true),
        (gay: i64, "", true, true),
    }
}
impl Command for HelloCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        // First parameter is the name to greet
        // let name = params
        //     .get(0)
        //     .cloned()
        //     .unwrap_or_else(|| "World".to_string());
        let name = WorldInput::parse(params.clone()).unwrap_or_default().name;
        WorldInput::parse(params).unwrap_or_default().gay;

        CommandResponse::new(true, TextMessage::new(format!("hello {}", name)))
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "hello",
            "A simple greeting command",
            WorldInput::param_description(),
        )
    }
}
