use crate::plugin::test_plugin::TestPlugin;
use crate::plugin_communication::app_starter::plugin_starter::ProgramController;
use async_trait::async_trait;
use std::io::Error;
use std::process::ExitStatus;
use tokio::io::{duplex, AsyncRead, AsyncWrite};

struct TestProgramController {
    stdin: Option<Box<dyn AsyncWrite + Unpin + Send + Sync>>,
    stdout: Option<Box<dyn AsyncRead + Unpin + Send + Sync>>,
}

impl TestProgramController {
    async fn new() -> Self {
        let (client, server) = duplex(1024);
        let (plugin_read, plugin_write) = tokio::io::split(client);
        let (server_read, server_write) = tokio::io::split(server);

        TestPlugin::new(Box::new(plugin_read), Box::new(plugin_write)).await;

        Self {
            stdout: Some(Box::new(server_read)),
            stdin: Some(Box::new(server_write)),
        }
    }
}

#[async_trait]
impl ProgramController for TestProgramController {
    fn get_stdin(&mut self) -> Result<Box<dyn AsyncWrite + Unpin + Send + Sync>, Error> {
        Ok(self.stdin.take().unwrap())
    }

    fn get_stdout(&mut self) -> Result<Box<dyn AsyncRead + Unpin + Send + Sync>, Error> {
        Ok(self.stdout.take().unwrap())
    }

    fn get_stderr(&mut self) -> Result<Box<dyn AsyncRead + Unpin + Send + Sync>, Error> {
        panic!("Not implemented")
    }

    fn is_running(&mut self) -> bool {
        panic!("Not implemented")
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        panic!("Not implemented")
    }

    async fn wait(&mut self) -> Result<ExitStatus, Error> {
        panic!("Not implemented")
    }
}
