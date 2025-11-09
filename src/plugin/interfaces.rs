use std::future::Future;
use crate::plugin::handlers::plugin_communicator::AsyncPluginCommunicator;
use crate::plugin::models;
use crate::plugin::models::{
    HandshakeRequest, HandshakeResponse, NormalRequest, NormalResponse, PluginConfig,
};
use models::Package;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use strum::Display;
use tokio::process::Child;
use tokio::sync::Mutex;

pub type CallbackFn = Box<dyn Fn(&[u8]) + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum PackageHandlerError {
    #[error("Sending Package failed")]
    SendingFailed(String),

    #[error("Serializing Package failed")]
    SerializationError(String),

    #[error("Process communication failed")]
    ProcessCommunicationError(String),

    #[error("Process shutdown failed")]
    ShutdownError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin startup failed: {0}")]
    StartupFailed(String),

    #[error("Plugin process failed: {0}")]
    ProcessError(String),

    #[error("Plugin communication failed: {0}")]
    CommunicationError(String),

    #[error("Plugin timeout: {0}")]
    Timeout(String),

    #[error("Plugin configuration error: {0}")]
    ConfigError(String),
}

pub trait PackageHandler: Sync + Send {
    fn send_package(&self, data: Vec<u8>) -> Result<(), PackageHandlerError>;

    fn set_callback_function(&mut self, callback: CallbackFn) -> Pin<Box< dyn Future<Output = ()>>>;

    fn start_reader_loop(&self);
}

pub trait PluginCommunicator {
    async fn send_request(
        &self,
        package: NormalRequest,
    ) -> Result<NormalResponse, PackageHandlerError>;

    fn send_package<T: Serialize>(&self, package: Package<T>) -> Result<(), PackageHandlerError>;

    async fn send_handshake(
        &self,
        package: HandshakeRequest,
    ) -> Result<HandshakeResponse, PackageHandlerError>;
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Display, Clone)]
pub enum State {
    Running,
    Starting,
    Error(String),
    Stopped,
}

pub struct Plugin {
    pub config: Arc<PluginConfig>,

    pub process: Arc<Mutex<Child>>,

    pub state: State,

    pub communicator: AsyncPluginCommunicator,

    pub config_dir: Box<PathBuf>,

    pub error_callback: Option<Arc<dyn Fn(&models::ErrorLog) + Send + Sync + 'static>>,
}
