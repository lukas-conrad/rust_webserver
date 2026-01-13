use crate::plugin::plugin_entry::PluginEntry;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait PluginStarter: Send + Sync {
    async fn start_app(
        &self,
        entry: &PluginEntry,
    ) -> Result<Box<dyn AppController>, std::io::Error>;
}

#[async_trait]
pub trait AppController: Send + Sync {
    fn get_stdin(&mut self) -> Result<Box<dyn AsyncWrite>, std::io::Error>;
    fn get_stdout(&mut self) -> Result<Box<dyn AsyncRead>, std::io::Error>;
    fn get_stderr(&mut self) -> Result<Box<dyn AsyncRead>, std::io::Error>;

    fn is_running(&self) -> bool;

    async fn shutdown(&mut self) -> Result<(), std::io::Error>;

    async fn wait(&mut self) -> Result<Option<i32>, std::io::Error>;

    async fn kill(&mut self) -> Result<(), std::io::Error>;
}
