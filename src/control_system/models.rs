pub struct CommandRequest {
    pub name: String,
    pub args: Vec<String>,
}

impl CommandRequest {
    pub fn new(name: String, args: Vec<String>) -> Self {
        Self { name, args }
    }
}

pub trait Message {
    fn to_string(&self) -> String;
    fn to_json(&self) -> String;
}

pub struct CommandResponse {
    pub success: bool,
    pub message: Box<dyn Message>,
}

impl CommandResponse {
    pub fn success(msg: impl Message + 'static) -> Self {
        CommandResponse::new(true, msg)
    }

    pub fn fail(msg: impl Message + 'static) -> Self {
        CommandResponse::new(false, msg)
    }

    pub fn new(success: bool, msg: impl Message + 'static) -> Self {
        Self {
            success,
            message: Box::new(msg),
        }
    }
}

pub struct ParameterDescriptor {
    pub name: String,
    pub description: String,
    pub required: bool,
}

impl ParameterDescriptor {
    pub fn new(name: String, description: String, required: bool) -> Self {
        Self {
            name,
            description,
            required,
        }
    }
}

pub struct CommandDescriptor {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterDescriptor>,
}

impl CommandDescriptor {
    pub fn new(name: String, description: String, parameters: Vec<ParameterDescriptor>) -> Self {
        Self {
            name,
            description,
            parameters,
        }
    }
}
