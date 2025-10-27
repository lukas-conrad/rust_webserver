use crate::controlSystem::utils::ParsableVariable;

pub struct CommandRequest {
    name: String,
    args: Vec<Param>,
}
pub struct CommandResponse {
    success: bool,
    message: String
}
pub struct CommandDescriptor {
    name: String,
    description: String
}
pub struct Param {
    key: String,
    value: dyn ParsableVariable
}