use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError;
use async_trait::async_trait;
use std::io::Error;
use std::process::ExitStatus;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait PluginStarter: Send + Sync {
    async fn start_app(
        &self,
        entry: &PluginEntry,
    ) -> Result<Box<dyn ProgramController>, PluginError>;
}

#[async_trait]
pub trait ProgramController: Send + Sync {
    async fn get_stdin(&self) -> Result<Box<dyn AsyncWrite + Unpin + Send + Sync>, Error>;
    async fn get_stdout(&self) -> Result<Box<dyn AsyncRead + Unpin + Send + Sync>, Error>;
    async fn get_stderr(&self) -> Result<Box<dyn AsyncRead + Unpin + Send + Sync>, Error>;

    async fn is_running(&self) -> bool;

    async fn shutdown(&self) -> Result<(), Error>;

    async fn wait(&self) -> Result<ExitStatus, Error>;
}
