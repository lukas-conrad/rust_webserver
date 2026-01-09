use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::protocol::{Filter, Listener, Protocol, ProtocolError};
use crate::plugin_communication::PluginCommunicator::PluginCommunicator;
use crate::plugin_old::models::Package;
use async_trait::async_trait;
use tokio::process::Child;

struct StdIoJsonProtocol {
    process: Child,
    listener: Option<Listener>
}

#[async_trait]
impl Protocol for StdIoJsonProtocol {
    async fn start_communication(&mut self, config: PluginEntry) -> Result<Box<dyn PluginCommunicator>, ProtocolError> {
        todo!()
    }
    

    async fn stop(&mut self) -> Result<(), ProtocolError> {
        todo!()
    }
}