use rust_webserver::io::data_storage::DataStorage;
use rust_webserver::io::in_memory_storage::InMemoryDataStorage;
use rust_webserver::plugin::plugin_config::{PluginConfig, ProtocolEnum};
use rust_webserver::plugin::plugin_manager::PluginManager;
use rust_webserver::plugin_communication::app_starter::test_plugin_starter::TestPluginStarter;
use rust_webserver::plugin_old::models::RequestInformation;
use std::path::Path;

#[tokio::test]
async fn plugin_manager_test() {
    let storage: Box<dyn DataStorage> = Box::new(InMemoryDataStorage::new());
    let config = create_test_plugin_config(
        "./test_plugin.exe".to_string(),
        "test/**/test_request".to_string(),
        "*.example.com".to_string(),
    );
    storage
        .store_data(
            serde_json::to_vec(&config).unwrap(),
            Path::new("test/test_plugin/plugin_config.json"),
        )
        .await
        .unwrap();

    let plugin_manager = PluginManager::new(storage, Box::new(TestPluginStarter::new().await));
}

fn create_test_plugin_config(
    startup_command: String,
    request_path: String,
    request_host: String,
) -> PluginConfig {
    PluginConfig {
        plugin_name: "Test Plugin".to_string(),
        startup_command,
        protocol: ProtocolEnum::StdIoJson,
        max_request_timeout: 1000,
        max_startup_time: 1000,
        request_information: RequestInformation {
            request_methods: vec!["*".to_string()],
            hosts: vec![request_host],
            paths: vec![request_path],
        },
    }
}
