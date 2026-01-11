use crate::plugin_communication::protocol::Protocol;
use crate::plugin_communication::std_io_json_protocol::StdIoJsonProtocol;
use crate::plugin_old::models::RequestInformation;
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    pub plugin_name: String,
    pub startup_command: String,
    pub protocol: ProtocolEnum,
    pub max_request_timeout: u64,
    pub max_startup_time: u64,
    pub request_information: RequestInformation,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProtocolEnum {
    StdIoJson,
}
impl ProtocolEnum {
    fn get_protocol(&self) -> Box<dyn Protocol> {
        match self {
            ProtocolEnum::StdIoJson => Box::new(StdIoJsonProtocol::new()),
        }
    }
}
