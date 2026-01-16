use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::app_starter::plugin_starter::{PluginStarter, ProgramController};
use crate::plugin_communication::plugin_communicator::{JsonCommunicator, PluginCommunicator};
use crate::plugin_communication::protocols::protocol::{Protocol, ProtocolError};
use async_trait::async_trait;

pub struct StdIoJsonProtocol {
    process: Option<Box<dyn ProgramController>>,
}

impl StdIoJsonProtocol {
    pub fn new() -> Self {
        Self { process: None }
    }
}

#[async_trait]
impl Protocol for StdIoJsonProtocol {
    async fn start_communication(
        &mut self,
        entry: &PluginEntry,
        plugin_starter: &Box<dyn PluginStarter>,
    ) -> Result<Box<dyn PluginCommunicator>, ProtocolError> {
        let mut controller = plugin_starter
            .start_app(entry)
            .await
            .map_err(|e| ProtocolError::StartupError(e.to_string()))?;

        let read = controller
            .get_stdout()
            .map_err(|e| ProtocolError::StartupError(e.to_string()))?;
        let write = controller
            .get_stdin()
            .map_err(|e| ProtocolError::StartupError(e.to_string()))?;
        let communicator = JsonCommunicator::new(read, write);

        self.process = Some(controller);

        Ok(Box::new(communicator))
    }

    async fn stop(&mut self) -> Result<(), ProtocolError> {
        // TODO: do not just end the app, send stop package
        if let Some(ref mut controller) = self.process {
            controller
                .shutdown()
                .await
                .map_err(|e| ProtocolError::StopError(e.to_string()))
        } else {
            Err(ProtocolError::StopError(
                "controller is not set".to_string(),
            ))
        }
    }
}
