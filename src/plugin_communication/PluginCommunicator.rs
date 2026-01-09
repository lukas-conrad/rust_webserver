use crate::plugin_communication::protocol::{Filter, Listener};
use crate::plugin_old::models::Package;
use async_trait::async_trait;

#[async_trait]
pub trait PluginCommunicator: Send + Sync {
    async fn set_listener(&mut self, listener: Listener);

    async fn send_package(&self, package: Package, filter: Filter);
}