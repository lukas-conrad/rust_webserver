use super::models::{CommandDescriptor, CommandRequest, CommandResponse};

trait ControlSystem {
    fn run_command(&self, request: CommandRequest) -> CommandResponse;
}

trait Command {
    fn execute(&self, params: Vec<(String, String)>) -> CommandResponse;
    fn get_command_descriptor(&self) -> CommandDescriptor;
}
