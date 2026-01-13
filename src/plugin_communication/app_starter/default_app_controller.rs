use crate::plugin_communication::app_starter::plugin_starter::AppController;
use async_trait::async_trait;
use std::io::{Error, ErrorKind};
use std::process::ExitStatus;
use tokio::process::Child;

pub struct DefaultAppController {
    process: Child,
}

impl DefaultAppController {
    pub fn new(process: Child) -> Self {
        Self { process }
    }
}

#[async_trait]
impl AppController for DefaultAppController {
    fn get_stdin(&mut self) -> Result<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>, Error> {
        let stdin = self
            .process
            .stdin
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "Could not extract stdin"))?;
        Ok(Box::new(stdin))
    }

    fn get_stdout(&mut self) -> Result<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>, Error> {
        let stdout = self
            .process
            .stdout
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "Could not extract stdout"))?;
        Ok(Box::new(stdout))
    }

    fn get_stderr(&mut self) -> Result<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>, Error> {
        let stderr = self
            .process
            .stderr
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "Could not extract stdout"))?;
        Ok(Box::new(stderr))
    }

    fn is_running(&mut self) -> bool {
        self.process.try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.process.kill().await
    }

    async fn wait(&mut self) -> Result<ExitStatus, Error> {
        self.process.wait().await
    }

}
