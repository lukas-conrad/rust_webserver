use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse, ParameterDescriptor};

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
