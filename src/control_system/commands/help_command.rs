use crate::control_system::commands::models::TextMessage;
use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse};

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
            // Skip the help command itself in the output (to avoid recursion)
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

        CommandResponse::new(true, TextMessage::new(help_text))
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "help",
            "Display all available commands and their descriptions",
            vec![],
        )
    }
}

