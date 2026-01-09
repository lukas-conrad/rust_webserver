use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::PluginCommunicator::PluginCommunicator;
use crate::plugin_old::models::Package;
use async_trait::async_trait;
use strum::Display;

#[derive(Display, Debug)]
pub enum ProtocolError {
    StartupError(String),
    StopError(String),
}
pub type Listener = Box<dyn Fn(Package) + Send + Sync>;
pub type Filter = Box<dyn Fn(Package) -> bool + Send + Sync>;

#[async_trait]
pub trait Protocol {
    async fn start_communication(
        &mut self,
        config: PluginEntry,
    ) -> Result<Box<dyn PluginCommunicator>, ProtocolError>;

    async fn stop(&mut self) -> Result<(), ProtocolError>;
}
