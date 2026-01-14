use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::app_starter::plugin_starter::PluginStarter;
use crate::plugin_communication::plugin_communicator::PluginCommunicator;
use async_trait::async_trait;
use strum::Display;

#[derive(Display, Debug)]
pub enum ProtocolError {
    StartupError(String),
    StopError(String),
}

#[async_trait]
pub trait Protocol: Send {
    async fn start_communication(
        &mut self,
        config: &PluginEntry,
        plugin_starter: &Box<dyn PluginStarter>
    ) -> Result<Box<dyn PluginCommunicator>, ProtocolError>;

    async fn stop(&mut self) -> Result<(), ProtocolError>;
}
