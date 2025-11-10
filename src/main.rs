extern crate core;

use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use log::{error, info};
use std::error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::net::TcpListener;

mod plugin;
mod webserver;
mod control_system;

use webserver::{WebServer, WebServerService};
use crate::plugin::PluginManager;
use crate::control_system::control_system::{ControlSystemWrapper, DefaultControlSystem};
use crate::control_system::cli::CommandLineInterface;

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error + Send + Sync>> {
    env_logger::init();
    info!("Starting modular webserver...");

    let plugins_dir = PathBuf::from("plugins");
    let error_log_dir = PathBuf::from("error_logs");

    for dir in &[&plugins_dir, &error_log_dir] {
        if !dir.exists() {
            fs::create_dir_all(dir).await?;
            info!("Created directory: {:?}", dir);
        }
    }

    let mut plugin_manager = Arc::new(PluginManager::new(error_log_dir));

    match plugin_manager.scan_plugins_directory(&plugins_dir).await {
        Ok(_) => info!("Successfully scanned plugins directory"),
        Err(e) => error!("Error scanning plugins directory: {}", e),
    }

    let server = Arc::new(WebServer::new(plugin_manager.clone()));

    let control_system = ControlSystemWrapper::new(DefaultControlSystem::new(plugin_manager.clone()));
    info!("Control System initialized");

    // Starte die CLI in einem separaten Thread
    let cli_control_system = control_system.clone();
    std::thread::spawn(move || {
        let cli = CommandLineInterface::new(Box::new(cli_control_system));
        cli.run();
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], 80));
    let listener = TcpListener::bind(addr).await?;
    info!("Webserver started on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let service = WebServerService {
            server: server.clone(),
        };

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                error!("Connection error: {:?}", err);
            }
        });
    }
}
