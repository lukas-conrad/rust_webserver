use futures::FutureExt;
use rust_webserver::plugin::test_plugin::TestPlugin;
use rust_webserver::plugin_communication::models::Package;
use std::process::exit;
use tokio::io::{stdin, stdout};

#[tokio::main]
async fn main() {
    let plugin_read = stdin();
    let plugin_write = stdout();
    TestPlugin::new(Box::new(plugin_read), Box::new(plugin_write), Some(
        Box::new(|package| {
            async move {
                match package {
                    Package::ShutdownRequest(request) => {
                        exit(0);
                    }
                    _ => None,
                }
            }
                .boxed()
        })
    )).await;

    std::future::pending::<()>().await;

    exit(0);
}
