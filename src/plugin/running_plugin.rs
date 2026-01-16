use crate::plugin::plugin_config::ProtocolEnum;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError;
use crate::plugin::plugin_manager::PluginError::PluginInitError;
use crate::plugin_communication::app_starter::plugin_starter::PluginStarter;
use crate::plugin_communication::plugin_communicator::{
    CommunicationError, Filter, Listener, PluginCommunicator,
};
use crate::plugin_communication::protocols::protocol::{Protocol, ProtocolError};
use crate::plugin_old::models::{HandshakeRequestContent, Package, PackageHandshakeResponse};
use std::io::Error;
use std::time::Duration;
use tokio::time::sleep;

pub struct RunningPlugin {
    communicator: Box<dyn PluginCommunicator>,
    protocol: Box<dyn Protocol>,
    protocol_enum: ProtocolEnum,
    request_timeout: u64,
    max_startup_time: u64,
    pub entry: PluginEntry,
}

impl RunningPlugin {
    pub async fn start_plugin(
        entry: &PluginEntry,
        plugin_starter: &Box<dyn PluginStarter>,
    ) -> Result<RunningPlugin, PluginError> {
        let protocol_enum = entry.config.protocol.clone();
        let mut protocol = protocol_enum.get_protocol();

        let communicator = protocol
            .start_communication(entry, plugin_starter)
            .await
            .map_err(|e| PluginError::PluginScanError(e.to_string()))?;

        let plugin = Self {
            communicator,
            protocol,
            request_timeout: entry.config.max_request_timeout,
            max_startup_time: entry.config.max_startup_time,
            protocol_enum: protocol_enum.clone(),
            entry: entry.clone(),
        };
        plugin.init_plugin().await?;
        Ok(plugin)
    }

    async fn init_plugin(&self) -> Result<(), PluginError> {
        let handshake_request = Package::HandshakeRequest(HandshakeRequestContent {
            protocol: self.protocol_enum.to_string(),
        });
        let response = self
            .communicator
            .send_package(&handshake_request, Some(PackageHandshakeResponse::filter()))
            .await
            .map_err(|e| PluginError::PluginInitError(e.to_string()))?
            .unwrap();

        if let Package::HandshakeResponse(content) = response {
            if content.response_code == 0 {
                Ok(())
            } else {
                Err(PluginInitError(format!(
                    "Plugin handshake error: {code}, {text}",
                    code = content.response_code,
                    text = content.response_code_text
                )))
            }
        } else {
            panic!("Wrong package returned")
        }
    }

    pub async fn send_package_with_response(
        &self,
        package: &Package,
        filter: Filter,
    ) -> Result<Package, CommunicationError> {
        let result = tokio::select! {
            result = self.communicator.send_package(&package, Some(filter)) => result,
            _ = sleep(Duration::from_millis(self.request_timeout)) => Err(CommunicationError::TimeoutError(format!("Timeout after {ms} milliseconds", ms = self.request_timeout)))
        }?;
        Ok(result.unwrap())
    }
    pub async fn send_package(&self, package: &Package) -> Result<(), CommunicationError> {
        let _ = self.communicator.send_package(&package, None).await?;
        Ok(())
    }

    pub async fn stop_plugin(&mut self) -> Result<(), ProtocolError> {
        self.protocol.stop().await
    }

    pub async fn set_listener(&mut self, listener: Listener) {
        self.communicator.set_listener(listener).await;
    }
}
