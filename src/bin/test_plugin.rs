use tokio::io::{stdin, stdout};
use rust_webserver::plugin::test_plugin::TestPlugin;

#[tokio::main]
async fn main() {
    let plugin_read = stdin();
    let plugin_write = stdout();
    let plugin = TestPlugin::new(Box::new(plugin_read), Box::new(plugin_write)).await;
}