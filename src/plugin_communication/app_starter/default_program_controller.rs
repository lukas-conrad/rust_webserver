use crate::plugin_communication::app_starter::plugin_starter::ProgramController;
use async_trait::async_trait;
use std::io::{Error, ErrorKind};
use std::process::ExitStatus;
use tokio::process::Child;
use tokio::sync::Mutex;

pub struct DefaultProgramController {
    process: Mutex<Child>,
}

impl DefaultProgramController {
    pub fn new(process: Child) -> Self {
        Self {
            process: Mutex::new(process),
        }
    }
}

#[async_trait]
impl ProgramController for DefaultProgramController {
    async fn get_stdin(
        &self,
    ) -> Result<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>, Error> {
        let stdin = self
            .process
            .lock()
            .await
            .stdin
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "Could not extract stdin"))?;
        Ok(Box::new(stdin))
    }

    async fn get_stdout(
        &self,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>, Error> {
        let stdout = self
            .process
            .lock()
            .await
            .stdout
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "Could not extract stdout"))?;
        Ok(Box::new(stdout))
    }

    async fn get_stderr(
        &self,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>, Error> {
        let stderr = self
            .process
            .lock()
            .await
            .stderr
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "Could not extract stdout"))?;
        Ok(Box::new(stderr))
    }

    async fn is_running(&self) -> bool {
        self.process
            .lock()
            .await
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }

    async fn shutdown(&self) -> Result<(), Error> {
        self.process.lock().await.kill().await
    }

    async fn wait(&self) -> Result<ExitStatus, Error> {
        self.process.lock().await.wait().await
    }
}
