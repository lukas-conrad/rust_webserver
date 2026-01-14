use crate::plugin::plugin_config::ProtocolEnum;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError;
use crate::plugin_communication::app_starter::plugin_starter::PluginStarter;
use crate::plugin_communication::plugin_communicator::{CommunicationError, Filter, Listener, PluginCommunicator};
use crate::plugin_communication::protocols::protocol::Protocol;
use crate::plugin_old::models::{HandshakeRequestContent, Package, PackageHandshakeResponse};
use futures::FutureExt;

pub struct RunningPlugin {
    communicator: Box<dyn PluginCommunicator>,
    protocol: Box<dyn Protocol>,
    pub protocol_enum: ProtocolEnum,
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
            protocol_enum: protocol_enum.clone(),
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

        if let Package::HandshakeResponse(content) = response {}

        Ok(())
    }

    // pub async fn send_package<T: Package>(package: &Package, filter: Option<Filter>) -> Result<T, CommunicationError> {
    //
    // }

    pub async fn set_listener(&mut self, listener: Listener) {
        self.communicator.set_listener(listener).await;
    }
}
