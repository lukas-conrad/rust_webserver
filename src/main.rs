extern crate core;

use crate::io::data_storage::FSDataStorage;
use clap::Parser;
use futures::FutureExt;
use log::{debug, error, info};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, error};
use tokio::fs;

mod control_system;
mod plugin_old;
mod webserver;

mod io;
mod plugin;
mod plugin_communication;
mod webserver_old;

use crate::plugin::plugin_manager::{PluginError, PluginManager, RequestHandler};
use crate::plugin_communication::app_starter::default_plugin_starter::DefaultPluginStarter;
use crate::webserver::http_1_server::Http1Server;
use crate::webserver::webserver::Webserver;

/// Simple HTTP/1.1 webserver with plugin system
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to bind the server to
    #[arg(short, long, default_value_t = 80)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error + Send + Sync>> {
    env_logger::init();

    let args = Args::parse();

    info!("Starting webserver on port {}...", args.port);

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
                    "Error when staring plugin {}. Error: {}",
                    entry.config.plugin_name, e
                )
            }
            _ => {}
        }
    }

    let plugin_manager: Arc<dyn RequestHandler> = Arc::new(plugin_manager);

    let server = Http1Server::start(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), args.port)).await;
    match server {
        Ok(server) => {
            server.set_listener(Box::new(move |request| {
                debug!("Received package {:?}", request);
                let plugin_manager = plugin_manager.clone();
                async move { plugin_manager.clone().route_request(request).await }.boxed()
            }));
        }
        Err(e) => {
            error!("Could not start webserver: {}", e.to_string());
            return Err(Box::from(e));
        }
    }

    tokio::signal::ctrl_c().await?;

    Ok(())
}
