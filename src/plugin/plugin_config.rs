use crate::plugin_old::models::RequestInformation;
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    pub plugin_name: String,
    pub startup_command: String,
    pub protocol: Protocol,
    pub max_request_timeout: u64,
    pub max_startup_time: u64,
    pub request_information: RequestInformation,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display)]
#[serde(rename_all = "UPPERCASE")]
pub enum Protocol {
    StdIoJson,
}
