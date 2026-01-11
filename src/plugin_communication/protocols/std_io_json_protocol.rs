use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::plugin_communicator::{Listener, PluginCommunicator};
use crate::plugin_communication::protocols::protocol::{Protocol, ProtocolError};
use async_trait::async_trait;
use tokio::process::Child;

pub struct StdIoJsonProtocol {
    process: Option<Child>,
}

impl StdIoJsonProtocol {
    pub(crate) fn new() -> Self {
        Self { process: None }
    }
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
