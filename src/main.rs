extern crate core;

use crate::io::data_storage::FSDataStorage;
use clap::Parser;
use futures::FutureExt;
use log::{debug, error, info};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::Arc;
use std::{env, error};
use tokio::fs;

mod webserver;

mod io;
mod plugin;
mod plugin_communication;
mod config;
use crate::plugin::plugin_manager::{PluginManager, RequestHandler};
use crate::plugin_communication::app_starter::default_plugin_starter::DefaultPluginStarter;
use crate::webserver::http_1_server::Http1Server;
use crate::webserver::https_1_server::Https1Server;
use crate::webserver::webserver::Webserver;
use crate::config::ServerConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Optional: Override config file path
    #[arg(short, long, default_value_t = String::from("config/config.json"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error + Send + Sync>> {
    std::panic::set_hook(Box::new(|panic_info| {
        let error_msg = format!("Panic occurred: {:?}", panic_info);
        eprintln!("{}", error_msg);

        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
        let log_path = format!("error_logs/panic_{}.log", timestamp);

        if let Err(e) = std::fs::write(&log_path, &error_msg) {
            eprintln!("Failed to write panic log to {}: {}", log_path, e);
        } else {
            eprintln!("Panic log written to {}", log_path);
        }

        exit(1);
    }));
    env_logger::init();

    let args = Args::parse();

    // Load configuration
    let server_config = match ServerConfig::load_or_create(&args.config) {
        Ok(config) => {
            info!("Server configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("Failed to load or create server configuration: {}", e);
            return Err(format!("Config error: {}", e).into());
        }
    };

    // Initialize HTTP and HTTPS servers based on configuration
    let mut http_server: Option<Arc<dyn Webserver>> = None;
    let mut https_server: Option<Arc<dyn Webserver>> = None;

    if server_config.http.enabled {
        info!(
            "Starting HTTP server on 0.0.0.0:{}",
            server_config.http.port
        );
        match Http1Server::start(SocketAddr::from(([0, 0, 0, 0], server_config.http.port))).await
        {
            Ok(server) => http_server = Some(server),
            Err(e) => {
                error!("Failed to start HTTP server: {}", e);
                return Err(format!("HTTP server error: {}", e).into());
            }
        }
    } else {
        info!("HTTP server is disabled in configuration");
    }

    if server_config.https.enabled {
        if server_config.https.domains.is_empty() {
            error!("HTTPS is enabled but no domains are configured");
            return Err("HTTPS requires at least one domain configuration".into());
        }

        info!(
            "Starting HTTPS server on 0.0.0.0:{} with {} domain(s)",
            server_config.https.port,
            server_config.https.domains.len()
        );
        match Https1Server::start(
            SocketAddr::from(([0, 0, 0, 0], server_config.https.port)),
            server_config.https.domains.clone(),
        )
        .await
        {
            Ok(server) => https_server = Some(server),
            Err(e) => {
                error!("Failed to start HTTPS server: {}", e);
                return Err(format!("HTTPS server error: {}", e).into());
            }
        }
    } else {
        info!("HTTPS server is disabled in configuration");
    }

    // Ensure at least one server is enabled
    if http_server.is_none() && https_server.is_none() {
        error!("Neither HTTP nor HTTPS is enabled in configuration");
        return Err("At least one server (HTTP or HTTPS) must be enabled".into());
    }

    info!("Starting webserver...");

    let plugins_dir = PathBuf::from("plugins");
    let error_log_dir = PathBuf::from("error_logs");

    for dir in &[&plugins_dir, &error_log_dir] {
        if !dir.exists() {
            fs::create_dir_all(dir).await?;
            info!("Created directory: {:?}", dir);
        }
    }

    let plugin_data_storage = FSDataStorage::new(env::current_dir()?.into_boxed_path());
    let plugin_starter = DefaultPluginStarter::new(Arc::new(plugin_data_storage.clone()));

    let mut plugin_manager =
        PluginManager::new(Box::new(plugin_data_storage), Box::new(plugin_starter));

    plugin_manager
        .scan_plugins(Path::new("plugins"))
        .await
        .unwrap();

    for entry in &plugin_manager.plugin_entries {
        let result = plugin_manager.start_plugin(entry).await;
        match result {
            Err(e) => {
                error!(
                    "Error when staring plugin {}. Error: {:?}",
                    entry.config.plugin_name, e
                )
            }
            _ => {}
        }
    }

    let plugin_manager = Arc::new(plugin_manager);

    // Set listener for all active servers
    let servers: Vec<Arc<dyn Webserver>> = vec![
        http_server.clone(),
        https_server.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    for server in servers {
        let plugin_manager_clone = plugin_manager.clone();
        server.set_listener(Box::new(move |request| {
            debug!("Received package {:?}", request);
            let plugin_manager_clone = plugin_manager_clone.clone();
            async move {
                plugin_manager_clone.clone().route_request(request).await
            }
            .boxed()
        }));
    }

    info!("All configured servers are running");

    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            info!("Stopping all plugins");

            plugin_manager.stop_plugins().await;

            info!("All Plugins stopped");
            exit(0);
        }
        Err(err) => {
            error!("Error when waiting for the Shutdown-Signal: {}", err);
        }
    }

    Ok(())
}
