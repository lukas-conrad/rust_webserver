use std::io::Error;
use std::process::ExitStatus;
use crate::plugin::plugin_entry::PluginEntry;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait PluginStarter: Send + Sync {
    async fn start_app(
        &self,
        entry: &PluginEntry,
    ) -> Result<Box<dyn AppController>, Error>;
}

#[async_trait]
pub trait AppController: Send + Sync {
    fn get_stdin(&mut self) -> Result<Box<dyn AsyncWrite + Unpin + Send + Sync>, Error>;
    fn get_stdout(&mut self) -> Result<Box<dyn AsyncRead + Unpin + Send + Sync>, Error>;
    fn get_stderr(&mut self) -> Result<Box<dyn AsyncRead + Unpin + Send + Sync>, Error>;

    fn is_running(&mut self) -> bool;

    async fn shutdown(&mut self) -> Result<(), Error>;

    async fn wait(&mut self) -> Result<ExitStatus, Error>;
}
