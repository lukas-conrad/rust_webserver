use crate::io::data_storage::FSBinding;
use crate::plugin::plugin_entry::PluginEntry;
use crate::plugin_communication::app_starter::default_program_controller::DefaultProgramController;
use crate::plugin_communication::app_starter::plugin_starter::{ProgramController, PluginStarter};
use async_trait::async_trait;
use std::io::Error;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;

pub struct DefaultPluginStarter {
    data_storage: Arc<dyn FSBinding>,
}

impl DefaultPluginStarter {
    pub fn new(data_storage: Arc<dyn FSBinding>) -> Self {
        Self { data_storage }
    }
}

#[async_trait]
impl PluginStarter for DefaultPluginStarter {
    async fn start_app(&self, entry: &PluginEntry) -> Result<Box<dyn ProgramController>, Error> {
        let dir = entry.path.parent().unwrap();

        #[cfg(target_os = "windows")]
        let process = Command::new("cmd")
            .arg("/C")
            .arg(&entry.config.startup_command)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        #[cfg(not(target_os = "windows"))]
        let process = Command::new("sh")
            .arg("-c")
            .arg(&entry.config.startup_command)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Box::new(DefaultProgramController::new(process)))
    }
}
