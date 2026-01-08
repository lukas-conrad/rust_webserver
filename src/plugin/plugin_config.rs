use serde::{Deserialize, Serialize};
use crate::plugin_old::models::RequestInformation;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    pub plugin_name: String,
    pub startup_command: String,
    pub protocols: Vec<String>,
    pub max_request_timeout: u64,
    pub max_startup_time: u64,
    pub request_information: RequestInformation,
}