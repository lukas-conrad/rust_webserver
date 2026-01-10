use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError;
use crate::plugin_communication::plugin_communicator::PluginCommunicator;
use crate::plugin_communication::protocol::Protocol;

pub struct RunningPlugin {
    entry: PluginEntry,
    communicator: Box<dyn PluginCommunicator>,
    protocol: Box<dyn Protocol>
}

impl RunningPlugin {
    async fn start_plugin(entry: PluginEntry) -> Result<RunningPlugin, PluginError> {
        todo!()
        
    }
}