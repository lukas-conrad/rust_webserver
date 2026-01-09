use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::protocol::{Protocol, ProtocolError};
use crate::plugin_communication::plugin_communicator::{Listener, PluginCommunicator};
use async_trait::async_trait;
use tokio::process::Child;

struct StdIoJsonProtocol {
    process: Child,
}

#[async_trait]
impl Protocol for StdIoJsonProtocol {
    async fn start_communication(
        &mut self,
        config: PluginEntry,
    ) -> Result<Box<dyn PluginCommunicator>, ProtocolError> {
        todo!("Start process and init communication")
    }

    async fn stop(&mut self) -> Result<(), ProtocolError> {
        todo!("Stop process")
    }
}
