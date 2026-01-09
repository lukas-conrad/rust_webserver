use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::plugin_communicator::PluginCommunicator;
use crate::plugin_old::models::Package;
use async_trait::async_trait;
use strum::Display;

#[derive(Display, Debug)]
pub enum ProtocolError {
    StartupError(String),
    StopError(String),
}

#[async_trait]
pub trait Protocol {
    async fn start_communication(
        &mut self,
        config: PluginEntry,
    ) -> Result<Box<dyn PluginCommunicator>, ProtocolError>;

    async fn stop(&mut self) -> Result<(), ProtocolError>;
}
