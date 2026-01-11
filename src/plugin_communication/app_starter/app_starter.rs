use async_trait::async_trait;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
trait AppStarter: Send + Sync {
    async fn start_app(&self, path: &Box<Path>) -> Result<Box<dyn AppController>, std::io::Error>;
}

#[async_trait]
pub trait AppController: Send + Sync {
    fn get_stream(&mut self) -> Box<dyn AsyncReadWrite>;

    fn is_running(&self) -> bool;

    async fn shutdown(&mut self) -> Result<(), std::io::Error>;

    async fn wait(&mut self) -> Result<Option<i32>, std::io::Error>;

    async fn kill(&mut self) -> Result<(), std::io::Error>;
}

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncReadWrite for T {}
