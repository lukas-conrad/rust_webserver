use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin::plugin_manager::PluginError;
use crate::plugin::test_plugin::{PackageListener, TestPlugin};
use crate::plugin_communication::app_starter::plugin_starter::{PluginStarter, ProgramController};
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::process::ExitStatus;
use tokio::io::{duplex, AsyncRead, AsyncWrite};

pub struct TestPluginProgramController {
    stdin: Option<Box<dyn AsyncWrite + Unpin + Send + Sync>>,
    stdout: Option<Box<dyn AsyncRead + Unpin + Send + Sync>>,
}

impl TestPluginProgramController {
    pub async fn new(listener: Option<PackageListener>) -> Self {
        let (client, server) = duplex(1024);
        let (plugin_read, plugin_write) = tokio::io::split(client);
        let (server_read, server_write) = tokio::io::split(server);

        TestPlugin::new(Box::new(plugin_read), Box::new(plugin_write), listener).await;

        Self {
            stdout: Some(Box::new(server_read)),
            stdin: Some(Box::new(server_write)),
        }
    }
}

#[async_trait]
impl ProgramController for TestPluginProgramController {
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

pub type TestPluginStartFunction =
    Box<dyn Fn() -> BoxFuture<'static, Box<dyn ProgramController>> + Send + Sync>;

pub struct TestPluginStarter {
    plugins: HashMap<String, TestPluginStartFunction>,
}

impl TestPluginStarter {
    pub async fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn add_plugin(
        &mut self,
        startup_command: String,
        plugin_start_function: TestPluginStartFunction,
    ) {
        self.plugins.insert(startup_command, plugin_start_function);
    }
}
#[async_trait]
impl PluginStarter for TestPluginStarter {
    async fn start_app(
        &self,
        entry: &PluginEntry,
    ) -> Result<Box<dyn ProgramController>, PluginError> {
        let startup_command = &entry.config.startup_command;
        let plugin = self.plugins.get(startup_command);
        if let Some(starter) = plugin {
            Ok(starter().await)
        } else {
            Err(PluginError::PluginStartError(
                "Could not find plugin to start".to_string(),
            ))
        }
    }
}
