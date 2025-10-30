use serde::Serialize;

pub struct CommandRequest {
    pub name: String,
    pub args: Vec<String>,
}

impl CommandRequest {
    pub fn new(name: String, args: Vec<String>) -> Self {
        Self { name, args }
    }
}

pub struct CommandResponse {
    pub success: bool,
    pub message: T,
}

impl CommandResponse {
    pub fn new(success: bool, message: ) -> Self {
        Self { success, message }
    }
}

pub struct ParameterDescriptor {
    pub name: String,
    pub description: String,
    pub required: bool,
}

impl ParameterDescriptor {
    pub fn new(name: String, description: String, required: bool) -> Self {
        Self { name, description, required }
    }
}

pub struct CommandDescriptor {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterDescriptor>,
}

impl CommandDescriptor {
    pub fn new(name: String, description: String, parameters: Vec<ParameterDescriptor>) -> Self {
        Self { name, description, parameters }
    }
}

