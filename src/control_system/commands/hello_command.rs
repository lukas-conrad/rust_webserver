use crate::control_system::commands::models::TextMessage;
use crate::control_system::control_system::Command;
use crate::control_system::models::{CommandDescriptor, CommandResponse, ParameterDescriptor};
use crate::param;

pub struct HelloCommand;

param! {
    HelloParams {
        positional: [
            (name: String, "The name to greet (positional parameter)")
        ],
        named: [
            (greeting: String, "Custom greeting word (e.g., --greeting=Hi)", false),
            (count: u32, "Number of times to repeat the greeting (e.g., --count=3)", false)
        ],
        flags: [
            (uppercase: bool, "Convert output to uppercase"),
            (exclaim: bool, "Add exclamation marks")
        ]
    }
}

impl Command for HelloCommand {
    fn execute(&self, params: Vec<String>) -> CommandResponse {
        let parsed = match HelloParams::parse(params) {
            Ok(p) => p,
            Err(e) => return CommandResponse::fail(TextMessage::new(format!("Error: {}", e)))
        };

        // Build the greeting - greeting ist Option<String>, also mit unwrap_or
        let greeting = parsed.greeting.unwrap_or_else(|| "Hello".to_string());

        // count ist Option<u32>, also mit unwrap_or
        let count = parsed.count.unwrap_or(1);

        let mut message = format!("{} {}", greeting, parsed.name);

        if parsed.exclaim {
            message.push_str("!");
        }

        // Repeat the message if count > 1
        if count > 1 {
            message = std::iter::repeat(message.as_str())
                .take(count as usize)
                .collect::<Vec<_>>()
                .join(" ");
        }

        if parsed.uppercase {
            message = message.to_uppercase();
        }

        CommandResponse::success(TextMessage::new(message))
    }

    fn get_command_descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::new(
            "hello",
            "A greeting command demonstrating positional, named, and flag parameters",
            HelloParams::param_description(),
        )
    }
}
