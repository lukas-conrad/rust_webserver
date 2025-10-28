use super::control_system::{ControlSystem, DefaultControlSystem};
use super::models::CommandRequest;
use log::{error};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

pub struct CommandLineInterface {
    control_system: Arc<DefaultControlSystem>,
}

impl CommandLineInterface {
    pub fn new(control_system: Arc<DefaultControlSystem>) -> Self {
        Self { control_system }
    }

    pub fn run(&self) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("> ");
            stdout.flush().unwrap();

            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let line = line.trim();
                    
                    if line.is_empty() {
                        continue;
                    }

                    self.process_command(line);
                }
                Err(e) => {
                    error!("Error reading input: {}", e);
                    break;
                }
            }
        }
    }

    fn process_command(&self, input: &str) {
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        if parts.is_empty() {
            return;
        }

        let command_name = parts[0].to_string();
        
        // Parse positionale Argumente - alle Teile nach dem Command-Namen
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

        let request = CommandRequest::new(command_name, args);
        let response = self.control_system.run_command(request);

        if response.success {
            println!("✓ {}", response.message);
        } else {
            println!("✗ {}", response.message);
        }
    }
}

