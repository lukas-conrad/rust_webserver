use crate::control_system::control_system::ControlSystem;
use crate::plugin_old::Plugin;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct PluginManager {
    pub plugins: Mutex<Vec<Plugin>>,
    working_directory: Box<Path>,
    error_log_path: Box<Path>,
}
impl PluginManager {
    fn new(working_directory: Box<Path>, error_log_path: Box<Path>) -> Self {
        Self {
            plugins: Mutex::new(vec![]),
            error_log_path,
            working_directory,
        }
    }
    
    
    
}
